use crate::app::pane::{Pane, PaneId};

pub type SplitId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SplitDirection {
    /// Divides height into top (first) and bottom (second) with a horizontal separator line.
    Horizontal,
    /// Divides width into left (first) and right (second) with a vertical separator line.
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusDirection {
    Left,
    Right,
    Up,
    Down,
}

pub const MIN_FONT_SIZE_SCALE: f32 = 0.2;
pub const MAX_FONT_SIZE_SCALE: f32 = 5.0;

/// Clamps a font size to between 20% and 500% of base font size, with a minimum of 1.0 px.
#[inline]
pub fn clamp_font_size(font_size: f32, base_font_size: f32) -> f32 {
    let base = if base_font_size > 0.0 {
        base_font_size
    } else {
        14.0
    };
    let min_size = (base * MIN_FONT_SIZE_SCALE).max(1.0);
    let max_size = (base * MAX_FONT_SIZE_SCALE).max(min_size);
    font_size.clamp(min_size, max_size)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaneRect {
    pub pane_id: PaneId,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub padding_x: f32,
    pub padding_y: f32,
    pub cols: usize,
    pub rows: usize,
    pub cell_width: f32,
    pub cell_height: f32,
}

impl PaneRect {
    #[inline]
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < (self.x + self.width) && py >= self.y && py < (self.y + self.height)
    }

    #[inline]
    pub fn center_x(&self) -> f32 {
        self.x + self.width * 0.5
    }

    #[inline]
    pub fn center_y(&self) -> f32 {
        self.y + self.height * 0.5
    }

    #[inline]
    pub fn text_x(&self) -> f32 {
        self.x + self.padding_x
    }

    #[inline]
    pub fn text_y(&self) -> f32 {
        self.y + self.padding_y
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SeparatorRect {
    pub split_id: SplitId,
    pub direction: SplitDirection,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub bounds_x: f32,
    pub bounds_y: f32,
    pub bounds_w: f32,
    pub bounds_h: f32,
}

impl SeparatorRect {
    #[inline]
    pub fn contains(&self, px: f32, py: f32) -> bool {
        self.hit_test(px, py, 3.0)
    }

    #[inline]
    pub fn hit_test(&self, px: f32, py: f32, hit_padding: f32) -> bool {
        let (min_x, max_x, min_y, max_y) = match self.direction {
            SplitDirection::Vertical => (
                self.x - hit_padding,
                self.x + self.width + hit_padding,
                self.y,
                self.y + self.height,
            ),
            SplitDirection::Horizontal => (
                self.x,
                self.x + self.width,
                self.y - hit_padding,
                self.y + self.height + hit_padding,
            ),
        };
        px >= min_x && px <= max_x && py >= min_y && py <= max_y
    }

    #[inline]
    pub fn is_adjacent_to_pane(&self, pane: &PaneRect) -> bool {
        self.active_segment_for_pane(pane).is_some()
    }

    #[inline]
    pub fn active_segment_for_pane(&self, pane: &PaneRect) -> Option<(f32, f32)> {
        match self.direction {
            SplitDirection::Vertical => {
                let touches_x = (pane.x + pane.width - self.x).abs() <= 1.5
                    || (self.x + self.width - pane.x).abs() <= 1.5;
                let start_y = self.y.max(pane.y);
                let end_y = (self.y + self.height).min(pane.y + pane.height);
                if touches_x && end_y > start_y + 0.5 {
                    Some((start_y, end_y))
                } else {
                    None
                }
            }
            SplitDirection::Horizontal => {
                let touches_y = (pane.y + pane.height - self.y).abs() <= 1.5
                    || (self.y + self.height - pane.y).abs() <= 1.5;
                let start_x = self.x.max(pane.x);
                let end_x = (self.x + self.width).min(pane.x + pane.width);
                if touches_y && end_x > start_x + 0.5 {
                    Some((start_x, end_x))
                } else {
                    None
                }
            }
        }
    }
}

#[allow(clippy::large_enum_variant)]
pub enum SplitNode {
    Pane(Pane),
    Split {
        id: SplitId,
        direction: SplitDirection,
        ratio: f32,
        first: Box<SplitNode>,
        second: Box<SplitNode>,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct SplitLayoutContext {
    pub sep_size: f32,
    pub padding_x: f32,
    pub padding_y: f32,
    pub base_cw_f: f32,
    pub base_ch_f: f32,
    pub base_font_size: f32,
    pub min_cols: usize,
    pub min_rows: usize,
}

impl SplitNode {
    #[inline]
    pub fn find_pane(&self, id: PaneId) -> Option<&Pane> {
        match self {
            Self::Pane(p) => {
                if p.id == id {
                    Some(p)
                } else {
                    None
                }
            }
            Self::Split { first, second, .. } => {
                first.find_pane(id).or_else(|| second.find_pane(id))
            }
        }
    }

    #[inline]
    pub fn find_pane_mut(&mut self, id: PaneId) -> Option<&mut Pane> {
        match self {
            Self::Pane(p) => {
                if p.id == id {
                    Some(p)
                } else {
                    None
                }
            }
            Self::Split { first, second, .. } => {
                if let Some(p) = first.find_pane_mut(id) {
                    Some(p)
                } else {
                    second.find_pane_mut(id)
                }
            }
        }
    }

    #[inline]
    pub fn first_pane(&self) -> Option<&Pane> {
        match self {
            Self::Pane(p) => Some(p),
            Self::Split { first, .. } => first.first_pane(),
        }
    }

    #[inline]
    pub fn first_pane_mut(&mut self) -> Option<&mut Pane> {
        match self {
            Self::Pane(p) => Some(p),
            Self::Split { first, .. } => first.first_pane_mut(),
        }
    }

    #[inline]
    pub fn last_pane(&self) -> Option<&Pane> {
        match self {
            Self::Pane(p) => Some(p),
            Self::Split { second, .. } => second.last_pane(),
        }
    }

    #[inline]
    pub fn last_pane_mut(&mut self) -> Option<&mut Pane> {
        match self {
            Self::Pane(p) => Some(p),
            Self::Split { second, .. } => second.last_pane_mut(),
        }
    }

    pub fn collect_panes<'a>(&'a self, list: &mut Vec<&'a Pane>) {
        match self {
            Self::Pane(p) => list.push(p),
            Self::Split { first, second, .. } => {
                first.collect_panes(list);
                second.collect_panes(list);
            }
        }
    }

    pub fn collect_panes_mut<'a>(&'a mut self, list: &mut Vec<&'a mut Pane>) {
        match self {
            Self::Pane(p) => list.push(p),
            Self::Split { first, second, .. } => {
                first.collect_panes_mut(list);
                second.collect_panes_mut(list);
            }
        }
    }

    pub fn for_each_pane_mut<F: FnMut(&mut Pane)>(&mut self, f: &mut F) {
        match self {
            Self::Pane(p) => f(p),
            Self::Split { first, second, .. } => {
                first.for_each_pane_mut(f);
                second.for_each_pane_mut(f);
            }
        }
    }

    pub fn collect_pane_ids(&self, list: &mut Vec<PaneId>) {
        match self {
            Self::Pane(p) => list.push(p.id),
            Self::Split { first, second, .. } => {
                first.collect_pane_ids(list);
                second.collect_pane_ids(list);
            }
        }
    }

    pub fn pane_count(&self) -> usize {
        match self {
            Self::Pane(_) => 1,
            Self::Split { first, second, .. } => first.pane_count() + second.pane_count(),
        }
    }

    pub fn remove_pane(&mut self, target_id: PaneId) -> Option<Pane> {
        let (first_is_target, second_is_target) = match self {
            Self::Split { first, second, .. } => (
                matches!(&**first, Self::Pane(p) if p.id == target_id),
                matches!(&**second, Self::Pane(p) if p.id == target_id),
            ),
            _ => (false, false),
        };

        if (first_is_target || second_is_target)
            && let Self::Split { first, second, .. } = self
        {
            let (target_box, keep_box) = if first_is_target {
                (first, second)
            } else {
                (second, first)
            };
            let keep = std::mem::replace(keep_box, Box::new(Self::Pane(Pane::dummy())));
            let target = std::mem::replace(target_box, Box::new(Self::Pane(Pane::dummy())));
            *self = *keep;
            if let Self::Pane(removed_pane) = *target {
                return Some(removed_pane);
            }
        }

        match self {
            Self::Split { first, second, .. } => {
                if let Some(removed) = first.remove_pane(target_id) {
                    Some(removed)
                } else {
                    second.remove_pane(target_id)
                }
            }
            Self::Pane(_) => None,
        }
    }

    pub fn set_split_ratio(&mut self, target_split_id: SplitId, new_ratio: f32) -> bool {
        match self {
            Self::Pane(_) => false,
            Self::Split {
                id,
                ratio,
                first,
                second,
                ..
            } => {
                if *id == target_split_id {
                    *ratio = new_ratio.clamp(0.05, 0.95);
                    true
                } else if first.set_split_ratio(target_split_id, new_ratio) {
                    true
                } else {
                    second.set_split_ratio(target_split_id, new_ratio)
                }
            }
        }
    }

    pub fn adjust_ancestor_split_ratio(
        &mut self,
        target_pane_id: PaneId,
        match_direction: SplitDirection,
        delta: f32,
    ) -> bool {
        match self {
            Self::Pane(_) => false,
            Self::Split {
                direction,
                ratio,
                first,
                second,
                ..
            } => {
                let target_child = if first.find_pane(target_pane_id).is_some() {
                    Some(&mut **first)
                } else if second.find_pane(target_pane_id).is_some() {
                    Some(&mut **second)
                } else {
                    None
                };

                if let Some(child) = target_child {
                    if child.adjust_ancestor_split_ratio(target_pane_id, match_direction, delta) {
                        return true;
                    }
                    if *direction == match_direction {
                        *ratio = (*ratio + delta).clamp(0.05, 0.95);
                        return true;
                    }
                }
                false
            }
        }
    }

    pub fn clear_unfocused_selections(&mut self, active_pane_id: PaneId) {
        match self {
            Self::Pane(p) => {
                if p.id != active_pane_id {
                    p.terminal.grid.selection.clear();
                    p.terminal.alt_grid.selection.clear();
                }
            }
            Self::Split { first, second, .. } => {
                first.clear_unfocused_selections(active_pane_id);
                second.clear_unfocused_selections(active_pane_id);
            }
        }
    }

    pub fn calculate_layout(
        &self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        ctx: &SplitLayoutContext,
        pane_rects: &mut Vec<PaneRect>,
        sep_rects: &mut Vec<SeparatorRect>,
    ) {
        match self {
            Self::Pane(pane) => {
                let scale = if ctx.base_font_size > 0.0 {
                    (pane.font_size / ctx.base_font_size)
                        .clamp(MIN_FONT_SIZE_SCALE, MAX_FONT_SIZE_SCALE)
                } else {
                    1.0
                };
                let pane_cw = (ctx.base_cw_f * scale).round().max(1.0);
                let pane_ch = (ctx.base_ch_f * scale).round().max(1.0);
                let text_w = (w - ctx.padding_x * 2.0).max(pane_cw);
                let text_h = (h - ctx.padding_y * 2.0).max(pane_ch);
                let cols = ((text_w / pane_cw).floor() as usize).max(1);
                let rows = ((text_h / pane_ch).floor() as usize).max(1);
                pane_rects.push(PaneRect {
                    pane_id: pane.id,
                    x,
                    y,
                    width: w,
                    height: h,
                    padding_x: ctx.padding_x,
                    padding_y: ctx.padding_y,
                    cols,
                    rows,
                    cell_width: pane_cw,
                    cell_height: pane_ch,
                });
            }
            Self::Split {
                id,
                direction,
                ratio,
                first,
                second,
            } => {
                let ratio = ratio.clamp(0.05, 0.95);
                match direction {
                    SplitDirection::Vertical => {
                        let avail_w = (w - ctx.sep_size).max(0.0);
                        let min_w =
                            ((ctx.min_cols as f32 * ctx.base_cw_f) + ctx.padding_x * 2.0).min(avail_w * 0.45);
                        let mut w1 = (avail_w * ratio).floor();
                        w1 = w1.clamp(min_w, (avail_w - min_w).max(min_w));
                        let w2 = (avail_w - w1).max(0.0);
                        let sep_x = x + w1;

                        sep_rects.push(SeparatorRect {
                            split_id: *id,
                            direction: *direction,
                            x: sep_x,
                            y,
                            width: ctx.sep_size,
                            height: h,
                            bounds_x: x,
                            bounds_y: y,
                            bounds_w: w,
                            bounds_h: h,
                        });

                        first.calculate_layout(x, y, w1, h, ctx, pane_rects, sep_rects);
                        second.calculate_layout(sep_x + ctx.sep_size, y, w2, h, ctx, pane_rects, sep_rects);
                    }
                    SplitDirection::Horizontal => {
                        let avail_h = (h - ctx.sep_size).max(0.0);
                        let min_h =
                            ((ctx.min_rows as f32 * ctx.base_ch_f) + ctx.padding_y * 2.0).min(avail_h * 0.45);
                        let mut h1 = (avail_h * ratio).floor();
                        h1 = h1.clamp(min_h, (avail_h - min_h).max(min_h));
                        let h2 = (avail_h - h1).max(0.0);
                        let sep_y = y + h1;

                        sep_rects.push(SeparatorRect {
                            split_id: *id,
                            direction: *direction,
                            x,
                            y: sep_y,
                            width: w,
                            height: ctx.sep_size,
                            bounds_x: x,
                            bounds_y: y,
                            bounds_w: w,
                            bounds_h: h,
                        });

                        first.calculate_layout(x, y, w, h1, ctx, pane_rects, sep_rects);
                        second.calculate_layout(x, sep_y + ctx.sep_size, w, h2, ctx, pane_rects, sep_rects);
                    }
                }
            }
        }
    }
}

pub struct SplitTree {
    pub root: SplitNode,
}

impl SplitTree {
    pub fn new(root_pane: Pane) -> Self {
        Self {
            root: SplitNode::Pane(root_pane),
        }
    }

    pub fn find_pane(&self, id: PaneId) -> Option<&Pane> {
        self.root.find_pane(id)
    }

    pub fn find_pane_mut(&mut self, id: PaneId) -> Option<&mut Pane> {
        self.root.find_pane_mut(id)
    }

    pub fn first_pane(&self) -> Option<&Pane> {
        self.root.first_pane()
    }

    pub fn first_pane_mut(&mut self) -> Option<&mut Pane> {
        self.root.first_pane_mut()
    }

    pub fn first_pane_id(&self) -> Option<PaneId> {
        self.root.first_pane().map(|p| p.id)
    }

    pub fn last_pane(&self) -> Option<&Pane> {
        self.root.last_pane()
    }

    pub fn last_pane_id(&self) -> Option<PaneId> {
        self.root.last_pane().map(|p| p.id)
    }

    pub fn collect_panes(&self) -> Vec<&Pane> {
        let mut list = Vec::new();
        self.root.collect_panes(&mut list);
        list
    }

    #[inline]
    pub fn panes(&self) -> Vec<&Pane> {
        self.collect_panes()
    }

    pub fn collect_panes_mut(&mut self) -> Vec<&mut Pane> {
        let mut list = Vec::new();
        self.root.collect_panes_mut(&mut list);
        list
    }

    pub fn for_each_pane_mut<F: FnMut(&mut Pane)>(&mut self, f: &mut F) {
        self.root.for_each_pane_mut(f);
    }

    #[inline]
    pub fn panes_mut(&mut self) -> Vec<&mut Pane> {
        self.collect_panes_mut()
    }

    pub fn collect_pane_ids(&self) -> Vec<PaneId> {
        let mut list = Vec::new();
        self.root.collect_pane_ids(&mut list);
        list
    }

    pub fn pane_count(&self) -> usize {
        self.root.pane_count()
    }

    pub fn split_pane(
        &mut self,
        target_id: PaneId,
        new_pane: Pane,
        direction: SplitDirection,
        ratio: f32,
        split_id: SplitId,
    ) -> bool {
        fn do_split(
            node: &mut SplitNode,
            target_id: PaneId,
            new_pane: Option<Pane>,
            direction: SplitDirection,
            ratio: f32,
            split_id: SplitId,
        ) -> (bool, Option<Pane>) {
            let mut pane_holder = new_pane;
            match node {
                SplitNode::Pane(p) => {
                    if p.id == target_id {
                        if let Some(new_p) = pane_holder.take() {
                            let old_p = std::mem::replace(p, Pane::dummy());
                            *node = SplitNode::Split {
                                id: split_id,
                                direction,
                                ratio: ratio.clamp(0.05, 0.95),
                                first: Box::new(SplitNode::Pane(old_p)),
                                second: Box::new(SplitNode::Pane(new_p)),
                            };
                            (true, None)
                        } else {
                            (false, None)
                        }
                    } else {
                        (false, pane_holder)
                    }
                }
                SplitNode::Split { first, second, .. } => {
                    let (ok_first, rem) =
                        do_split(first, target_id, pane_holder, direction, ratio, split_id);
                    if ok_first {
                        (true, None)
                    } else {
                        do_split(second, target_id, rem, direction, ratio, split_id)
                    }
                }
            }
        }

        let (success, _) = do_split(
            &mut self.root,
            target_id,
            Some(new_pane),
            direction,
            ratio,
            split_id,
        );
        success
    }

    pub fn remove_pane(&mut self, target_id: PaneId) -> Option<Pane> {
        if let SplitNode::Pane(p) = &self.root
            && p.id == target_id
        {
            return None;
        }
        self.root.remove_pane(target_id)
    }

    pub fn set_split_ratio(&mut self, split_id: SplitId, new_ratio: f32) -> bool {
        self.root.set_split_ratio(split_id, new_ratio)
    }

    pub fn adjust_ancestor_split_ratio(&mut self, target_pane_id: PaneId, delta: f32) -> bool {
        self.root
            .adjust_ancestor_split_ratio(target_pane_id, SplitDirection::Horizontal, delta)
            || self.root.adjust_ancestor_split_ratio(
                target_pane_id,
                SplitDirection::Vertical,
                delta,
            )
    }

    pub fn adjust_ancestor_split_ratio_with_direction(
        &mut self,
        target_pane_id: PaneId,
        direction: SplitDirection,
        delta: f32,
    ) -> bool {
        self.root
            .adjust_ancestor_split_ratio(target_pane_id, direction, delta)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn calculate_layout(
        &self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        separator_size: f32,
        padding_x: f32,
        padding_y: f32,
        base_cw: u32,
        base_ch: u32,
        base_font_size: f32,
        min_cols: usize,
        min_rows: usize,
    ) -> (Vec<PaneRect>, Vec<SeparatorRect>) {
        let mut pane_rects = Vec::with_capacity(8);
        let mut sep_rects = Vec::with_capacity(8);
        let ctx = SplitLayoutContext {
            sep_size: separator_size,
            padding_x,
            padding_y,
            base_cw_f: base_cw.max(1) as f32,
            base_ch_f: base_ch.max(1) as f32,
            base_font_size,
            min_cols,
            min_rows,
        };
        self.root
            .calculate_layout(x, y, width, height, &ctx, &mut pane_rects, &mut sep_rects);
        (pane_rects, sep_rects)
    }

    pub fn clear_unfocused_selections(&mut self, active_pane_id: PaneId) {
        self.root.clear_unfocused_selections(active_pane_id);
    }
}

/// Find the nearest neighbor pane in a given 2D direction.
pub fn find_neighbor_pane(
    rects: &[PaneRect],
    active_id: PaneId,
    direction: FocusDirection,
) -> Option<PaneId> {
    let active_rect = rects.iter().find(|r| r.pane_id == active_id)?;

    let mut best_candidate: Option<(PaneId, f32)> = None;

    let active_cx = active_rect.center_x();
    let active_cy = active_rect.center_y();

    for target in rects {
        if target.pane_id == active_id {
            continue;
        }

        let target_cx = target.center_x();
        let target_cy = target.center_y();

        let (is_in_direction, primary_dist, overlap, perp_dist) = match direction {
            FocusDirection::Left => {
                let valid = target.x + target.width <= active_rect.x + 2.0;
                let dist = active_rect.x - (target.x + target.width);
                let overlap = (active_rect.y + active_rect.height).min(target.y + target.height)
                    - active_rect.y.max(target.y);
                let perp = (active_cy - target_cy).abs();
                (valid, dist, overlap, perp)
            }
            FocusDirection::Right => {
                let valid = target.x >= active_rect.x + active_rect.width - 2.0;
                let dist = target.x - (active_rect.x + active_rect.width);
                let overlap = (active_rect.y + active_rect.height).min(target.y + target.height)
                    - active_rect.y.max(target.y);
                let perp = (active_cy - target_cy).abs();
                (valid, dist, overlap, perp)
            }
            FocusDirection::Up => {
                let valid = target.y + target.height <= active_rect.y + 2.0;
                let dist = active_rect.y - (target.y + target.height);
                let overlap = (active_rect.x + active_rect.width).min(target.x + target.width)
                    - active_rect.x.max(target.x);
                let perp = (active_cx - target_cx).abs();
                (valid, dist, overlap, perp)
            }
            FocusDirection::Down => {
                let valid = target.y >= active_rect.y + active_rect.height - 2.0;
                let dist = target.y - (active_rect.y + active_rect.height);
                let overlap = (active_rect.x + active_rect.width).min(target.x + target.width)
                    - active_rect.x.max(target.x);
                let perp = (active_cx - target_cx).abs();
                (valid, dist, overlap, perp)
            }
        };

        if !is_in_direction {
            continue;
        }

        let primary_clamped = primary_dist.max(0.0);
        let score = if overlap > 0.0 {
            primary_clamped * 1000.0 + perp_dist - overlap * 10.0
        } else {
            primary_clamped * 1000.0 + perp_dist * 2000.0
        };

        if let Some((_, best_score)) = best_candidate {
            if score < best_score {
                best_candidate = Some((target.pane_id, score));
            }
        } else {
            best_candidate = Some((target.pane_id, score));
        }
    }

    best_candidate.map(|(id, _)| id)
}
