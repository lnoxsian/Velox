use velox::window::x11::shm::{
    MAX_X11_BUFFER_BYTES, MAX_X11_BUFFER_DIMENSION, ShmSegmentGuard, X11BufferError,
    X11Framebuffer, validate_x11_buffer_dimensions,
};

#[test]
fn test_validate_x11_buffer_dimensions_valid() {
    // 800x600 (SVGA)
    let layout = validate_x11_buffer_dimensions(800, 600).expect("valid 800x600 layout");
    assert_eq!(layout.width, 800);
    assert_eq!(layout.height, 600);
    assert_eq!(layout.pixel_count, 480_000);
    assert_eq!(layout.num_bytes, 1_920_000);
    assert_eq!(layout.alloc_bytes, 2_400_000); // 25% extra headroom

    // 1920x1080 (Full HD)
    let layout_fhd = validate_x11_buffer_dimensions(1920, 1080).expect("valid 1080p layout");
    assert_eq!(layout_fhd.width, 1920);
    assert_eq!(layout_fhd.height, 1080);
    assert_eq!(layout_fhd.pixel_count, 2_073_600);
    assert_eq!(layout_fhd.num_bytes, 8_294_400);
    assert_eq!(layout_fhd.alloc_bytes, 10_368_000);

    // 7680x4320 (8K UHD)
    let layout_8k = validate_x11_buffer_dimensions(7680, 4320).expect("valid 8K layout");
    assert_eq!(layout_8k.width, 7680);
    assert_eq!(layout_8k.height, 4320);
    assert_eq!(layout_8k.pixel_count, 33_177_600);
    assert_eq!(layout_8k.num_bytes, 132_710_400);
    assert_eq!(layout_8k.alloc_bytes, 165_888_000);
    assert!(layout_8k.alloc_bytes <= MAX_X11_BUFFER_BYTES);
}

#[test]
fn test_validate_x11_buffer_dimensions_zero_rejected() {
    match validate_x11_buffer_dimensions(0, 600) {
        Err(X11BufferError::ZeroDimension {
            width: 0,
            height: 600,
        }) => {}
        other => panic!("Expected ZeroDimension, got: {:?}", other),
    }

    match validate_x11_buffer_dimensions(800, 0) {
        Err(X11BufferError::ZeroDimension {
            width: 800,
            height: 0,
        }) => {}
        other => panic!("Expected ZeroDimension, got: {:?}", other),
    }

    match validate_x11_buffer_dimensions(0, 0) {
        Err(X11BufferError::ZeroDimension {
            width: 0,
            height: 0,
        }) => {}
        other => panic!("Expected ZeroDimension, got: {:?}", other),
    }
}

#[test]
fn test_validate_x11_buffer_dimensions_maximum_allowed() {
    let layout_w = validate_x11_buffer_dimensions(MAX_X11_BUFFER_DIMENSION, 1)
        .expect("max width single line is valid");
    assert_eq!(layout_w.width, MAX_X11_BUFFER_DIMENSION);
    assert_eq!(layout_w.height, 1);

    let layout_h = validate_x11_buffer_dimensions(1, MAX_X11_BUFFER_DIMENSION)
        .expect("max height single column is valid");
    assert_eq!(layout_h.width, 1);
    assert_eq!(layout_h.height, MAX_X11_BUFFER_DIMENSION);
}

#[test]
fn test_validate_x11_buffer_dimensions_oversized_dimension_rejected() {
    let over = MAX_X11_BUFFER_DIMENSION + 1;
    match validate_x11_buffer_dimensions(over, 600) {
        Err(X11BufferError::DimensionTooLarge {
            width,
            height: 600,
            max,
        }) => {
            assert_eq!(width, over);
            assert_eq!(max, MAX_X11_BUFFER_DIMENSION);
        }
        other => panic!("Expected DimensionTooLarge, got: {:?}", other),
    }

    match validate_x11_buffer_dimensions(800, over) {
        Err(X11BufferError::DimensionTooLarge {
            width: 800,
            height,
            max,
        }) => {
            assert_eq!(height, over);
            assert_eq!(max, MAX_X11_BUFFER_DIMENSION);
        }
        other => panic!("Expected DimensionTooLarge, got: {:?}", other),
    }
}

#[test]
fn test_validate_x11_buffer_dimensions_framebuffer_byte_limit_rejected() {
    // 16384 x 16384 is each within MAX_X11_BUFFER_DIMENSION, but size is 1 GiB > 256 MiB
    match validate_x11_buffer_dimensions(16384, 16384) {
        Err(X11BufferError::BufferTooLarge { size, max }) => {
            assert_eq!(size, 1024 * 1024 * 1024);
            assert_eq!(max, MAX_X11_BUFFER_BYTES);
        }
        other => panic!("Expected BufferTooLarge, got: {:?}", other),
    }
}

#[test]
fn test_validate_x11_buffer_dimensions_overflow_rejected() {
    assert!(validate_x11_buffer_dimensions(u32::MAX, 1).is_err());
    assert!(validate_x11_buffer_dimensions(1, u32::MAX).is_err());
    assert!(validate_x11_buffer_dimensions(u32::MAX / 2, 2).is_err());
    assert!(validate_x11_buffer_dimensions((u32::MAX / 4) + 1, 1).is_err());
}

#[test]
fn test_x11_shm_guard_cleanup_on_failure() {
    let size_bytes = 4096;
    let shm_id = unsafe { libc::shmget(libc::IPC_PRIVATE, size_bytes, libc::IPC_CREAT | 0o600) };
    assert!(shm_id >= 0, "libc::shmget must succeed");

    let addr = unsafe { libc::shmat(shm_id, std::ptr::null(), 0) };
    assert_ne!(
        addr, -1isize as *mut libc::c_void,
        "libc::shmat must succeed"
    );

    // Verify segment is attached initially
    let mut shm_ds: libc::shmid_ds = unsafe { std::mem::zeroed() };
    let ctl_res = unsafe { libc::shmctl(shm_id, libc::IPC_STAT, &mut shm_ds) };
    assert_eq!(ctl_res, 0);
    assert_eq!(shm_ds.shm_nattch, 1);

    // Guard will drop at end of scope without being disarmed
    {
        let _guard = ShmSegmentGuard { shm_id, addr };
    }

    // Now verify the segment is detached (shm_nattch == 0) and destroyed
    let mut post_ds: libc::shmid_ds = unsafe { std::mem::zeroed() };
    let stat_res = unsafe { libc::shmctl(shm_id, libc::IPC_STAT, &mut post_ds) };
    // After IPC_RMID and detach, either IPC_STAT fails with EINVAL or nattch is 0
    if stat_res == 0 {
        assert_eq!(post_ds.shm_nattch, 0, "segment must be detached");
    }
}

#[test]
fn test_x11_framebuffer_heap_fallback_behavior() {
    let mut fb = X11Framebuffer::new_software(100, 100, 24).expect("valid software framebuffer");
    assert_eq!(fb.pixels_mut().len(), 10_000);
    fb.pixels_mut()[0] = 0xFF112233;

    // Validate that validate_x11_buffer_dimensions rejects zero and oversized dimensions
    assert!(validate_x11_buffer_dimensions(0, 100).is_err());
    assert!(validate_x11_buffer_dimensions(16385, 100).is_err());
    assert!(validate_x11_buffer_dimensions(16384, 16384).is_err());

    // Buffer and dimensions remain intact
    assert_eq!(fb.width, 100);
    assert_eq!(fb.height, 100);
    assert_eq!(fb.pixel_count, 10_000);
    assert_eq!(fb.pixels_mut()[0], 0xFF112233);
}
