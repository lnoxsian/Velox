use velox::window::DamageRect;
use velox::window::wayland::shm::{
    AcquireAction, BufferLifecycleState, BufferState, MAX_WAYLAND_BUFFER_BYTES,
    MAX_WAYLAND_BUFFER_DIMENSION, ReleaseAction, ShmBufferInitGuard, WaylandBufferError,
    validate_buffer_dimensions,
};

#[test]
fn test_buffer_lifecycle_acquire_and_submit() {
    let mut lifecycle = BufferLifecycleState::<u32>::new(3);

    // Initial state: empty pool
    assert_eq!(lifecycle.active_count(), 0);
    assert_eq!(lifecycle.available_count(), 0);
    assert_eq!(lifecycle.compositor_owned_count(), 0);

    // Frame 1: acquire first buffer
    let (action1, destroys1) = lifecycle.acquire(800, 600);
    assert_eq!(action1, AcquireAction::AllocateNew);
    assert!(destroys1.is_empty());
    lifecycle.register_new(101, 800, 600);
    assert_eq!(lifecycle.active_count(), 1);

    // Submit Frame 1 to compositor
    lifecycle.submit(&101);
    assert_eq!(lifecycle.compositor_owned_count(), 1);
    assert_eq!(lifecycle.available_count(), 0);

    // Frame 2: acquire second buffer (double buffering)
    let (action2, destroys2) = lifecycle.acquire(800, 600);
    assert_eq!(action2, AcquireAction::AllocateNew);
    assert!(destroys2.is_empty());
    lifecycle.register_new(102, 800, 600);
    lifecycle.submit(&102);
    assert_eq!(lifecycle.compositor_owned_count(), 2);
    assert_eq!(lifecycle.active_count(), 2);

    // Frame 3: acquire third buffer (triple buffering)
    let (action3, destroys3) = lifecycle.acquire(800, 600);
    assert_eq!(action3, AcquireAction::AllocateNew);
    assert!(destroys3.is_empty());
    lifecycle.register_new(103, 800, 600);
    lifecycle.submit(&103);
    assert_eq!(lifecycle.compositor_owned_count(), 3);
    assert_eq!(lifecycle.active_count(), 3);
}

#[test]
fn test_buffer_lifecycle_no_available_buffer_deferral() {
    let mut lifecycle = BufferLifecycleState::<u32>::new(3);

    // Allocate and submit 3 buffers so the pool is completely exhausted
    for id in [1, 2, 3] {
        let (action, _) = lifecycle.acquire(800, 600);
        assert_eq!(action, AcquireAction::AllocateNew);
        lifecycle.register_new(id, 800, 600);
        lifecycle.submit(&id);
    }
    assert_eq!(lifecycle.compositor_owned_count(), 3);
    assert_eq!(lifecycle.available_count(), 0);

    // Frame 4: all 3 buffers are currently CompositorOwned -> must defer
    let (action_deferred, destroys) = lifecycle.acquire(800, 600);
    assert_eq!(action_deferred, AcquireAction::NoAvailableBuffer);
    assert!(destroys.is_empty());

    // Compositor releases buffer 2
    let release_action = lifecycle.on_release(&2);
    assert_eq!(release_action, ReleaseAction::MakeAvailable);
    assert_eq!(lifecycle.available_count(), 1);
    assert_eq!(lifecycle.compositor_owned_count(), 2);

    // Retry acquire: buffer 2 is now available and reused without allocation
    let (action_retry, destroys_retry) = lifecycle.acquire(800, 600);
    assert_eq!(action_retry, AcquireAction::Reuse(2));
    assert!(destroys_retry.is_empty());
    assert_eq!(lifecycle.active_buffers[1].state, BufferState::Acquired);

    // Submit buffer 2 again
    lifecycle.submit(&2);
    assert_eq!(lifecycle.compositor_owned_count(), 3);
}

#[test]
fn test_buffer_lifecycle_resize_preserves_compositor_owned() {
    let mut lifecycle = BufferLifecycleState::<u32>::new(3);

    // Buffer 1 (800x600) acquired and submitted (CompositorOwned)
    let (action1, _) = lifecycle.acquire(800, 600);
    assert_eq!(action1, AcquireAction::AllocateNew);
    lifecycle.register_new(1, 800, 600);
    lifecycle.submit(&1);

    // Buffer 2 (800x600) acquired, submitted, and then released (Available)
    let (action2, _) = lifecycle.acquire(800, 600);
    assert_eq!(action2, AcquireAction::AllocateNew);
    lifecycle.register_new(2, 800, 600);
    lifecycle.submit(&2);
    assert_eq!(lifecycle.on_release(&2), ReleaseAction::MakeAvailable);

    assert_eq!(lifecycle.active_count(), 2);
    assert_eq!(lifecycle.available_count(), 1);
    assert_eq!(lifecycle.compositor_owned_count(), 1);

    // User resizes window to 1024x768
    // Buffer 2 (Available) can be destroyed immediately.
    // Buffer 1 (CompositorOwned) must NOT be destroyed; it must be retired until release!
    let (action_resize, immediate_destroys) = lifecycle.acquire(1024, 768);
    assert_eq!(action_resize, AcquireAction::AllocateNew);
    assert_eq!(immediate_destroys, vec![2]); // Buffer 2 destroyed immediately

    // Buffer 1 must now be in retired_buffers awaiting release
    assert_eq!(lifecycle.retired_count(), 1);
    assert_eq!(lifecycle.retired_buffers[0].id, 1);
    assert!(lifecycle.retired_buffers[0].pending_destroy);

    // Allocate and submit new resized Buffer 3 (1024x768)
    lifecycle.register_new(3, 1024, 768);
    lifecycle.submit(&3);
    assert_eq!(lifecycle.active_count(), 1);

    // Compositor finally releases old Buffer 1 (800x600)
    let release_old = lifecycle.on_release(&1);
    assert_eq!(release_old, ReleaseAction::Destroy);
    assert_eq!(lifecycle.retired_count(), 0); // Cleanly removed from retired buffers
}

#[test]
fn test_buffer_lifecycle_multiple_resizes_queue_retired() {
    let mut lifecycle = BufferLifecycleState::<u32>::new(3);

    // Buffer 1 at 800x600 (CompositorOwned)
    lifecycle.acquire(800, 600);
    lifecycle.register_new(1, 800, 600);
    lifecycle.submit(&1);

    // Resize 1: 900x650
    lifecycle.acquire(900, 650);
    lifecycle.register_new(2, 900, 650);
    lifecycle.submit(&2);

    // Resize 2: 1000x700
    lifecycle.acquire(1000, 700);
    lifecycle.register_new(3, 1000, 700);
    lifecycle.submit(&3);

    // Both Buffer 1 and Buffer 2 must be in retired_buffers awaiting release
    assert_eq!(lifecycle.retired_count(), 2);
    assert_eq!(lifecycle.active_count(), 1); // Only Buffer 3 is active

    // Release Buffer 2 first (out of order release)
    assert_eq!(lifecycle.on_release(&2), ReleaseAction::Destroy);
    assert_eq!(lifecycle.retired_count(), 1);

    // Release Buffer 1
    assert_eq!(lifecycle.on_release(&1), ReleaseAction::Destroy);
    assert_eq!(lifecycle.retired_count(), 0);

    // Active Buffer 3 is unaffected
    assert_eq!(lifecycle.active_count(), 1);
    assert_eq!(lifecycle.on_release(&3), ReleaseAction::MakeAvailable);
    assert_eq!(lifecycle.available_count(), 1);
}

#[test]
fn test_damage_rect_union() {
    let rect_a = DamageRect {
        x: 10,
        y: 20,
        width: 100,
        height: 50,
    };
    let rect_b = DamageRect {
        x: 50,
        y: 60,
        width: 80,
        height: 40,
    };

    let union = rect_a.union(rect_b);
    assert_eq!(union.x, 10);
    assert_eq!(union.y, 20);
    assert_eq!(union.width, 120); // max_x = 130, min_x = 10 -> 120
    assert_eq!(union.height, 80); // max_y = 100, min_y = 20 -> 80

    // Union with zero-sized rect returns the non-empty rect
    let empty = DamageRect {
        x: 0,
        y: 0,
        width: 0,
        height: 0,
    };
    assert_eq!(rect_a.union(empty), rect_a);
    assert_eq!(empty.union(rect_b), rect_b);
}

#[test]
fn test_validate_buffer_dimensions_valid_resolutions() {
    // 800x600 (SVGA)
    let layout = validate_buffer_dimensions(800, 600).expect("valid 800x600 layout");
    assert_eq!(layout.width, 800);
    assert_eq!(layout.height, 600);
    assert_eq!(layout.stride, 3200);
    assert_eq!(layout.pixel_count, 480_000);
    assert_eq!(layout.size_bytes, 1_920_000);
    assert_eq!(layout.i32_width, 800);
    assert_eq!(layout.i32_height, 600);
    assert_eq!(layout.i32_stride, 3200);
    assert_eq!(layout.i32_size, 1_920_000);

    // 1920x1080 (Full HD)
    let layout_fhd = validate_buffer_dimensions(1920, 1080).expect("valid 1080p layout");
    assert_eq!(layout_fhd.stride, 7680);
    assert_eq!(layout_fhd.pixel_count, 2_073_600);
    assert_eq!(layout_fhd.size_bytes, 8_294_400);

    // 7680x4320 (8K UHD)
    let layout_8k = validate_buffer_dimensions(7680, 4320).expect("valid 8K layout");
    assert_eq!(layout_8k.stride, 30720);
    assert_eq!(layout_8k.pixel_count, 33_177_600);
    assert_eq!(layout_8k.size_bytes, 132_710_400);
    assert!(layout_8k.size_bytes <= MAX_WAYLAND_BUFFER_BYTES);
}

#[test]
fn test_validate_buffer_dimensions_zero_rejected() {
    match validate_buffer_dimensions(0, 600) {
        Err(WaylandBufferError::ZeroDimension {
            width: 0,
            height: 600,
        }) => {}
        other => panic!("Expected ZeroDimension, got: {:?}", other),
    }

    match validate_buffer_dimensions(800, 0) {
        Err(WaylandBufferError::ZeroDimension {
            width: 800,
            height: 0,
        }) => {}
        other => panic!("Expected ZeroDimension, got: {:?}", other),
    }

    match validate_buffer_dimensions(0, 0) {
        Err(WaylandBufferError::ZeroDimension {
            width: 0,
            height: 0,
        }) => {}
        other => panic!("Expected ZeroDimension, got: {:?}", other),
    }
}

#[test]
fn test_validate_buffer_dimensions_dimension_limit_rejected() {
    let over = MAX_WAYLAND_BUFFER_DIMENSION + 1;
    match validate_buffer_dimensions(over, 600) {
        Err(WaylandBufferError::DimensionTooLarge {
            width,
            height: 600,
            max,
        }) => {
            assert_eq!(width, over);
            assert_eq!(max, MAX_WAYLAND_BUFFER_DIMENSION);
        }
        other => panic!("Expected DimensionTooLarge, got: {:?}", other),
    }

    match validate_buffer_dimensions(800, over) {
        Err(WaylandBufferError::DimensionTooLarge {
            width: 800,
            height,
            max,
        }) => {
            assert_eq!(height, over);
            assert_eq!(max, MAX_WAYLAND_BUFFER_DIMENSION);
        }
        other => panic!("Expected DimensionTooLarge, got: {:?}", other),
    }
}

#[test]
fn test_validate_buffer_dimensions_framebuffer_byte_limit_rejected() {
    // 16384 x 16384 <= MAX_WAYLAND_BUFFER_DIMENSION, but size is 1 GiB > 256 MiB
    match validate_buffer_dimensions(16384, 16384) {
        Err(WaylandBufferError::BufferTooLarge { size, max }) => {
            assert_eq!(size, 1024 * 1024 * 1024);
            assert_eq!(max, MAX_WAYLAND_BUFFER_BYTES);
        }
        other => panic!("Expected BufferTooLarge, got: {:?}", other),
    }
}

#[test]
fn test_validate_buffer_dimensions_overflow_rejected() {
    // Huge values that would overflow arithmetic
    assert!(validate_buffer_dimensions(u32::MAX, 1).is_err());
    assert!(validate_buffer_dimensions(1, u32::MAX).is_err());
    assert!(validate_buffer_dimensions(u32::MAX / 2, 2).is_err());
}

#[test]
fn test_shm_init_guard_drop_cleans_resources() {
    let name = b"velox-guard-test\0";
    let fd = unsafe { libc::memfd_create(name.as_ptr() as *const libc::c_char, libc::MFD_CLOEXEC) };
    assert!(fd >= 0, "memfd_create must succeed");

    let size_bytes = 4096;
    let trunc_res = unsafe { libc::ftruncate(fd, size_bytes as libc::off_t) };
    assert_eq!(trunc_res, 0);

    let mem = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            size_bytes,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_SHARED,
            fd,
            0,
        )
    };
    assert_ne!(mem, libc::MAP_FAILED);

    // Verify fd is valid before guard drop
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    assert!(flags >= 0, "fd should be valid and open");

    // Scope the guard so it drops at end of scope
    {
        let _guard = ShmBufferInitGuard {
            fd,
            mem,
            size_bytes,
            pool: None,
            buffer: None,
        };
    }

    // Now verify fd was closed upon drop
    let post_flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    assert_eq!(post_flags, -1, "fd should be closed after guard drop");
    let err = std::io::Error::last_os_error();
    assert_eq!(
        err.raw_os_error(),
        Some(libc::EBADF),
        "closing fd must result in EBADF"
    );
}

#[test]
fn test_rapid_wayland_resizes_churn_and_pool_invariants() {
    let mut lifecycle = BufferLifecycleState::<u32>::new(3);

    let resolutions = [
        (640, 480),
        (800, 600),
        (1024, 768),
        (1920, 1080),
        (1366, 768),
        (2560, 1440),
        (801, 603), // odd dimensions
        (1280, 720),
        (1920, 1200),
        (800, 600),
    ];

    let mut next_id = 1000u32;
    let mut compositor_owned: Vec<u32> = Vec::new();
    let mut active_registered: std::collections::HashSet<u32> = std::collections::HashSet::new();

    for i in 0..1000 {
        let (w, h) = resolutions[i % resolutions.len()];

        let (action, immediate_destroys) = lifecycle.acquire(w, h);

        // Track immediate destroys
        for destroyed_id in immediate_destroys {
            active_registered.remove(&destroyed_id);
        }

        match action {
            AcquireAction::AllocateNew => {
                let id = next_id;
                next_id += 1;
                lifecycle.register_new(id, w, h);
                active_registered.insert(id);
                lifecycle.submit(&id);
                compositor_owned.push(id);
            }
            AcquireAction::Reuse(id) => {
                lifecycle.submit(&id);
                compositor_owned.push(id);
            }
            AcquireAction::NoAvailableBuffer => {
                // Deferral is valid when all 3 buffers are compositor-owned
                assert_eq!(lifecycle.compositor_owned_count(), 3);
            }
        }

        // Active pool count must never exceed max_buffers (3)
        assert!(
            lifecycle.active_count() <= 3,
            "Active buffer count {} exceeded max_buffers 3 on iteration {}",
            lifecycle.active_count(),
            i
        );

        // Simulate intermittent compositor releases (e.g. every other frame)
        if i % 2 == 0 && !compositor_owned.is_empty() {
            let rel_idx = (i * 7) % compositor_owned.len();
            let rel_id = compositor_owned.remove(rel_idx);
            let rel_action = lifecycle.on_release(&rel_id);
            match rel_action {
                ReleaseAction::Destroy => {
                    active_registered.remove(&rel_id);
                }
                ReleaseAction::MakeAvailable => {
                    assert!(active_registered.contains(&rel_id));
                }
                ReleaseAction::Ignored => {}
            }
        }
    }

    // Drain all remaining compositor-owned buffers
    for id in compositor_owned {
        let rel_action = lifecycle.on_release(&id);
        if rel_action == ReleaseAction::Destroy {
            active_registered.remove(&id);
        }
    }

    assert_eq!(
        lifecycle.compositor_owned_count(),
        0,
        "All buffers must be drained from compositor ownership"
    );
    assert!(
        lifecycle.active_count() <= 3,
        "Active count must stay within pool bounds after drain"
    );
}

#[test]
fn test_compositor_buffer_release_out_of_order_and_delayed_sequencing() {
    let mut lifecycle = BufferLifecycleState::<u32>::new(3);

    // Acquire and submit 3 buffers at 800x600
    for id in [1, 2, 3] {
        let (action, _) = lifecycle.acquire(800, 600);
        assert_eq!(action, AcquireAction::AllocateNew);
        lifecycle.register_new(id, 800, 600);
        lifecycle.submit(&id);
    }
    assert_eq!(lifecycle.compositor_owned_count(), 3);
    assert_eq!(lifecycle.active_count(), 3);

    // Resize to 1024x768: all 3 become retired since they are currently CompositorOwned
    let (action_res, destroys) = lifecycle.acquire(1024, 768);
    assert_eq!(action_res, AcquireAction::AllocateNew);
    assert!(
        destroys.is_empty(),
        "Compositor-owned buffers cannot be destroyed immediately"
    );

    lifecycle.register_new(4, 1024, 768);
    lifecycle.submit(&4);

    // Verify out-of-order release: release 3 first, then 1, then 2
    assert_eq!(lifecycle.on_release(&3), ReleaseAction::Destroy);
    assert_eq!(lifecycle.on_release(&1), ReleaseAction::Destroy);
    assert_eq!(lifecycle.on_release(&2), ReleaseAction::Destroy);

    // Re-releasing already destroyed buffers must be ignored safely
    assert_eq!(lifecycle.on_release(&3), ReleaseAction::Ignored);
    assert_eq!(lifecycle.on_release(&1), ReleaseAction::Ignored);
    assert_eq!(lifecycle.on_release(&9999), ReleaseAction::Ignored);

    // Release current active buffer 4: it was not retired, so it becomes Available
    assert_eq!(lifecycle.on_release(&4), ReleaseAction::MakeAvailable);
    assert_eq!(lifecycle.available_count(), 1);
    assert_eq!(lifecycle.compositor_owned_count(), 0);

    // Acquiring 1024x768 again reuses buffer 4 without reallocation
    let (reuse_action, _) = lifecycle.acquire(1024, 768);
    assert_eq!(reuse_action, AcquireAction::Reuse(4));
}
