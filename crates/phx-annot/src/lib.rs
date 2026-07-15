//! Annotation documents with interval tiers, point tiers, typed tier relations,
//! integrity validation, and invertible label, boundary, point, and tier-order
//! mutations.
#![warn(missing_docs)]

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;

/// Stable identifier for a tier in an annotation document.
#[derive(Clone, Copy, Debug, Hash, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct TierId(u64);

impl TierId {
    /// Wraps a raw value as a tier identifier for importer-built documents.
    ///
    /// Importers that assemble a document with [`Annotation::from_raw`] mint the
    /// identifiers their tiers reference. [`Annotation::validate`] reports any
    /// duplicate or dangling identifier introduced through this constructor.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the numeric value carried by this stable identifier.
    pub fn get(self) -> u64 {
        self.0
    }
}

/// Stable identifier for a boundary shared by adjacent intervals.
#[derive(Clone, Copy, Debug, Hash, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct BoundaryId(u64);

impl BoundaryId {
    /// Wraps a raw value as a boundary identifier for importer-built documents.
    ///
    /// Adjacent intervals in a tier share one boundary identifier; importers
    /// building a document with [`Annotation::from_raw`] assign the same value to
    /// the end of one interval and the start of the next. [`Annotation::validate`]
    /// reports a continuity mismatch when they differ.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the numeric value carried by this stable identifier.
    pub fn get(self) -> u64 {
        self.0
    }

    /// Builds the inverse operation for a successful `Annotation::insert_boundary` call.
    ///
    /// The original `tier` and `at` arguments are not needed because the boundary
    /// identifier locates the split. If the split was performed on aligned tiers,
    /// applying the inverse removes the corresponding aligned boundaries atomically.
    pub fn inverse_insert(self) -> InverseMutation {
        InverseMutation::RemoveBoundary { boundary: self }
    }
}

/// Stable identifier for an interval.
#[derive(Clone, Copy, Debug, Hash, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct IntervalId(u64);

impl IntervalId {
    /// Wraps a raw value as an interval identifier for importer-built documents.
    ///
    /// Importers that assemble a document with [`Annotation::from_raw`] mint the
    /// identifiers their intervals carry. [`Annotation::validate`] reports any
    /// duplicate identifier introduced through this constructor.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the numeric value carried by this stable identifier.
    pub fn get(self) -> u64 {
        self.0
    }
}

/// Stable identifier for a point annotation.
#[derive(Clone, Copy, Debug, Hash, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct PointId(u64);

impl PointId {
    /// Wraps a raw value as a point identifier for importer-built documents.
    ///
    /// Importers that assemble a document with [`Annotation::from_raw`] mint the
    /// identifiers their points carry. [`Annotation::validate`] reports any
    /// duplicate identifier introduced through this constructor.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the numeric value carried by this stable identifier.
    pub fn get(self) -> u64 {
        self.0
    }
}

/// Annotation document with ordered tiers and a finite time domain in seconds.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Annotation {
    xmin: f64,
    xmax: f64,
    tiers: Vec<TierSlot>,
    next_tier_id: u64,
    next_boundary_id: u64,
    next_interval_id: u64,
    next_point_id: u64,
}

impl PartialEq for Annotation {
    fn eq(&self, other: &Self) -> bool {
        self.xmin.to_bits() == other.xmin.to_bits()
            && self.xmax.to_bits() == other.xmax.to_bits()
            && self.tiers == other.tiers
    }
}

impl Annotation {
    /// Creates an empty document over the finite time domain `[xmin, xmax]`.
    pub fn new(xmin: f64, xmax: f64) -> Result<Self, AnnotationError> {
        if !xmin.is_finite() {
            return Err(AnnotationError::NonFiniteTime {
                value: xmin,
                context: TimeRole::DomainStart,
            });
        }
        if !xmax.is_finite() {
            return Err(AnnotationError::NonFiniteTime {
                value: xmax,
                context: TimeRole::DomainEnd,
            });
        }
        if xmin >= xmax {
            return Err(AnnotationError::InvalidTimeDomain { xmin, xmax });
        }
        Ok(Self {
            xmin,
            xmax,
            tiers: Vec::new(),
            next_tier_id: 1,
            next_boundary_id: 1,
            next_interval_id: 1,
            next_point_id: 1,
        })
    }

    /// Builds a document from raw tiers and recomputes identifier generators.
    ///
    /// This constructor accepts invalid tier contents so importers can preserve
    /// source data and call `validate` to report every issue.
    pub fn from_raw(xmin: f64, xmax: f64, tiers: Vec<TierSlot>) -> Self {
        let mut annotation = Self {
            xmin,
            xmax,
            tiers,
            next_tier_id: 1,
            next_boundary_id: 1,
            next_interval_id: 1,
            next_point_id: 1,
        };
        annotation.reseed_ids();
        annotation
    }

    /// Returns the start of the document time domain in seconds.
    pub fn xmin(&self) -> f64 {
        self.xmin
    }

    /// Returns the end of the document time domain in seconds.
    pub fn xmax(&self) -> f64 {
        self.xmax
    }

    /// Returns tiers in their explicit document order.
    pub fn tiers(&self) -> &[TierSlot] {
        &self.tiers
    }

    /// Returns a tier slot by stable tier identifier.
    pub fn tier(&self, id: TierId) -> Option<&TierSlot> {
        self.tiers.iter().find(|slot| slot.id == id)
    }

    /// Adds an interval tier containing one unlabeled interval covering the document domain.
    pub fn add_interval_tier(
        &mut self,
        name: &str,
        relation: TierRelation,
    ) -> Result<TierId, AnnotationError> {
        let mut candidate = self.clone();
        let id = candidate.alloc_tier_id();
        let start = candidate.alloc_boundary_id();
        let end = candidate.alloc_boundary_id();
        let interval = candidate.alloc_interval_id();
        candidate.tiers.push(TierSlot {
            id,
            relation,
            tier: Tier::Interval(IntervalTier {
                name: name.to_owned(),
                intervals: vec![Interval {
                    id: interval,
                    start_boundary: start,
                    end_boundary: end,
                    xmin: candidate.xmin,
                    xmax: candidate.xmax,
                    label: String::new(),
                }],
            }),
        });
        candidate.commit_if_valid()?;
        *self = candidate;
        Ok(id)
    }

    /// Adds a point tier with sorted, strictly increasing point times.
    pub fn add_point_tier(
        &mut self,
        name: &str,
        points: Vec<(f64, String)>,
        relation: TierRelation,
    ) -> Result<TierId, AnnotationError> {
        let mut candidate = self.clone();
        let id = candidate.alloc_tier_id();
        let mut stored = Vec::with_capacity(points.len());
        for (time, label) in points {
            reject_control_label(&label)?;
            stored.push(Point {
                id: candidate.alloc_point_id(),
                time,
                label,
            });
        }
        candidate.tiers.push(TierSlot {
            id,
            relation,
            tier: Tier::Point(PointTier {
                name: name.to_owned(),
                points: stored,
            }),
        });
        candidate.commit_if_valid()?;
        *self = candidate;
        Ok(id)
    }

    /// Reports all integrity issues that can be found without panicking.
    pub fn validate(&self) -> Vec<IntegrityIssue> {
        let mut issues = Vec::new();
        if !self.xmin.is_finite() {
            issues.push(IntegrityIssue::NonFiniteTime {
                value: self.xmin,
                context: TimeRole::DomainStart,
            });
        }
        if !self.xmax.is_finite() {
            issues.push(IntegrityIssue::NonFiniteTime {
                value: self.xmax,
                context: TimeRole::DomainEnd,
            });
        }
        if self.xmin >= self.xmax {
            issues.push(IntegrityIssue::InvalidTimeDomain {
                xmin: self.xmin,
                xmax: self.xmax,
            });
        }

        let mut tier_ids = HashSet::new();
        let mut interval_ids = HashSet::new();
        let mut point_ids = HashSet::new();
        let mut boundary_ids = HashSet::new();

        for slot in &self.tiers {
            if !tier_ids.insert(slot.id) {
                issues.push(IntegrityIssue::DuplicateTierId { tier: slot.id });
            }
            match &slot.tier {
                Tier::Interval(tier) => self.validate_interval_tier(
                    slot.id,
                    tier,
                    &mut interval_ids,
                    &mut boundary_ids,
                    &mut issues,
                ),
                Tier::Point(tier) => {
                    self.validate_point_tier(slot.id, tier, &mut point_ids, &mut issues)
                }
            }
        }

        for slot in &self.tiers {
            match slot.relation {
                TierRelation::Independent => {}
                TierRelation::AlignedBoundaries { with } => {
                    self.validate_aligned_relation(slot.id, with, &mut issues);
                }
                TierRelation::ChildOf { parent } => {
                    self.validate_child_relation(slot.id, parent, &mut issues);
                }
            }
        }

        issues
    }

    /// Splits one interval tier boundary, or all aligned peers, at `at`.
    pub fn insert_boundary(
        &mut self,
        tier: TierId,
        at: f64,
    ) -> Result<BoundaryId, AnnotationError> {
        if !at.is_finite() {
            return Err(AnnotationError::NonFiniteTime {
                value: at,
                context: TimeRole::Boundary,
            });
        }
        let tier_indices = self.aligned_interval_indices_for_tier(tier)?;
        let mut candidate = self.clone();
        let mut returned = None;
        for index in tier_indices {
            let boundary = candidate.split_interval_at_index(index, at)?;
            if candidate.tiers[index].id == tier {
                returned = Some(boundary);
            }
        }
        candidate.commit_if_valid()?;
        let boundary = returned.ok_or(AnnotationError::UnknownTier { tier })?;
        *self = candidate;
        Ok(boundary)
    }

    /// Moves a boundary while preserving tier order and relation integrity.
    pub fn move_boundary(
        &mut self,
        id: BoundaryId,
        to: f64,
        mode: AlignMode,
    ) -> Result<Moved, AnnotationError> {
        if !to.is_finite() {
            return Err(AnnotationError::NonFiniteTime {
                value: to,
                context: TimeRole::Boundary,
            });
        }
        let (tier, at) = self.boundary_location(id)?;
        let indices = match mode {
            AlignMode::Linked => self.aligned_interval_indices_for_tier(tier)?,
            AlignMode::SingleTier => {
                if self.aligned_interval_indices_for_tier(tier)?.len() > 1 {
                    return Err(AnnotationError::AlignedBoundaryRequiresLinkedMode {
                        boundary: id,
                    });
                }
                vec![self.tier_index(tier)?]
            }
        };
        let mut candidate = self.clone();
        let mut moves = Vec::new();
        for index in indices {
            let moved = candidate.move_boundary_in_tier_at(index, at, to)?;
            moves.push(moved);
        }
        candidate.commit_if_valid()?;
        *self = candidate;
        Ok(Moved { moves })
    }

    /// Removes an interior boundary by merging adjacent intervals.
    pub fn remove_boundary(&mut self, id: BoundaryId) -> Result<Merged, AnnotationError> {
        let (tier, at) = self.boundary_location(id)?;
        let indices = self.aligned_interval_indices_for_tier(tier)?;
        let mut candidate = self.clone();
        let mut merges = Vec::new();
        for index in indices {
            merges.push(candidate.merge_boundary_in_tier_at(index, at)?);
        }
        candidate.commit_if_valid()?;
        *self = candidate;
        Ok(Merged { merges })
    }

    /// Replaces a label after rejecting C0 control characters and `DEL`.
    pub fn set_label(
        &mut self,
        target: LabelTarget,
        text: &str,
    ) -> Result<LabelChange, AnnotationError> {
        reject_control_label(text)?;
        let mut candidate = self.clone();
        let old_text = candidate.replace_label(target, text)?;
        candidate.commit_if_valid()?;
        *self = candidate;
        Ok(LabelChange {
            target,
            old_text,
            new_text: text.to_owned(),
        })
    }

    /// Inserts a point into a point tier at its time-sorted position.
    pub fn insert_point(
        &mut self,
        tier: TierId,
        time: f64,
        label: &str,
    ) -> Result<PointInsertion, AnnotationError> {
        if !time.is_finite() {
            return Err(AnnotationError::NonFiniteTime {
                value: time,
                context: TimeRole::Point,
            });
        }
        reject_control_label(label)?;
        let mut candidate = self.clone();
        let id = candidate.alloc_point_id();
        let point = candidate.insert_point_into_tier(
            tier,
            Point {
                id,
                time,
                label: label.to_owned(),
            },
        )?;
        candidate.commit_if_valid()?;
        *self = candidate;
        Ok(PointInsertion { tier, point })
    }

    /// Moves a point to `to` seconds, keeping its stable identifier.
    pub fn move_point(&mut self, id: PointId, to: f64) -> Result<PointMoved, AnnotationError> {
        if !to.is_finite() {
            return Err(AnnotationError::NonFiniteTime {
                value: to,
                context: TimeRole::Point,
            });
        }
        let mut candidate = self.clone();
        let (tier, from) = candidate.move_point_time(id, to)?;
        candidate.commit_if_valid()?;
        *self = candidate;
        Ok(PointMoved {
            tier,
            point: id,
            from,
            to,
        })
    }

    /// Removes a point by stable identifier.
    pub fn remove_point(&mut self, id: PointId) -> Result<PointRemoval, AnnotationError> {
        let mut candidate = self.clone();
        let (tier, point) = candidate.remove_point_by_id(id)?;
        candidate.commit_if_valid()?;
        *self = candidate;
        Ok(PointRemoval { tier, point })
    }

    /// Moves a tier to `to_index` in document order, keeping every stable id.
    ///
    /// `to_index` is clamped to the last position. Tier relations reference
    /// stable ids, not positions, so reordering never changes document validity.
    pub fn reorder_tier(
        &mut self,
        tier: TierId,
        to_index: usize,
    ) -> Result<TierReorder, AnnotationError> {
        let from_index = self.tier_index(tier)?;
        let to_index = to_index.min(self.tiers.len() - 1);
        let mut candidate = self.clone();
        let slot = candidate.tiers.remove(from_index);
        candidate.tiers.insert(to_index, slot);
        candidate.commit_if_valid()?;
        *self = candidate;
        Ok(TierReorder {
            tier,
            from_index,
            to_index,
        })
    }

    /// Searches interval and point labels and returns byte spans for every match.
    pub fn search(&self, query: &LabelQuery) -> Vec<Hit> {
        let matcher = match query.matcher() {
            Some(matcher) => matcher,
            None => return Vec::new(),
        };
        let tier_filter: Option<HashSet<TierId>> = query
            .tiers
            .as_ref()
            .map(|tiers| tiers.iter().copied().collect());
        let mut hits = Vec::new();
        for slot in &self.tiers {
            if tier_filter
                .as_ref()
                .is_some_and(|filter| !filter.contains(&slot.id))
            {
                continue;
            }
            match &slot.tier {
                Tier::Interval(tier) => {
                    for interval in &tier.intervals {
                        for span in matcher.find_all(&interval.label) {
                            hits.push(Hit {
                                tier: slot.id,
                                target: LabelTarget::Interval {
                                    tier: slot.id,
                                    interval: interval.id,
                                },
                                span,
                            });
                        }
                    }
                }
                Tier::Point(tier) => {
                    for point in &tier.points {
                        for span in matcher.find_all(&point.label) {
                            hits.push(Hit {
                                tier: slot.id,
                                target: LabelTarget::Point {
                                    tier: slot.id,
                                    point: point.id,
                                },
                                span,
                            });
                        }
                    }
                }
            }
        }
        hits
    }

    /// Applies a stored inverse mutation.
    pub fn apply_inverse(&mut self, inverse: &InverseMutation) -> Result<(), AnnotationError> {
        match inverse {
            InverseMutation::RemoveBoundary { boundary } => {
                self.remove_boundary(*boundary)?;
            }
            InverseMutation::MoveBoundaries { moves } => {
                let mut candidate = self.clone();
                for movement in moves {
                    let index = candidate.tier_index(movement.tier)?;
                    let moved =
                        candidate.move_boundary_in_tier_at(index, movement.to, movement.from)?;
                    if moved.boundary != movement.boundary {
                        return Err(AnnotationError::UnknownBoundary {
                            boundary: movement.boundary,
                        });
                    }
                }
                candidate.commit_if_valid()?;
                *self = candidate;
            }
            InverseMutation::RestoreMergedBoundary { merged } => {
                let mut candidate = self.clone();
                for merge in &merged.merges {
                    candidate.restore_merge(merge)?;
                }
                candidate.commit_if_valid()?;
                *self = candidate;
            }
            InverseMutation::SetLabel { target, text } => {
                self.set_label(*target, text)?;
            }
            InverseMutation::RemovePoint { point } => {
                self.remove_point(*point)?;
            }
            InverseMutation::RestorePoint { tier, point } => {
                let mut candidate = self.clone();
                candidate.insert_point_into_tier(*tier, point.clone())?;
                candidate.reseed_ids();
                candidate.commit_if_valid()?;
                *self = candidate;
            }
            InverseMutation::MovePoint { point, to } => {
                self.move_point(*point, *to)?;
            }
            InverseMutation::ReorderTier { tier, to_index } => {
                self.reorder_tier(*tier, *to_index)?;
            }
        }
        Ok(())
    }

    fn validate_interval_tier(
        &self,
        tier_id: TierId,
        tier: &IntervalTier,
        interval_ids: &mut HashSet<IntervalId>,
        boundary_ids: &mut HashSet<BoundaryId>,
        issues: &mut Vec<IntegrityIssue>,
    ) {
        if tier.intervals.is_empty() {
            issues.push(IntegrityIssue::IntervalTierEmpty { tier: tier_id });
            return;
        }

        let mut tier_boundaries = Vec::with_capacity(tier.intervals.len() + 1);
        let first = &tier.intervals[0];
        if !same_time(first.xmin, self.xmin) {
            issues.push(IntegrityIssue::DomainStartMismatch {
                tier: tier_id,
                found: first.xmin,
                expected: self.xmin,
            });
        }
        tier_boundaries.push(first.start_boundary);

        for (index, interval) in tier.intervals.iter().enumerate() {
            if !interval_ids.insert(interval.id) {
                issues.push(IntegrityIssue::DuplicateIntervalId {
                    tier: tier_id,
                    interval: interval.id,
                });
            }
            if !interval.xmin.is_finite() {
                issues.push(IntegrityIssue::NonFiniteTime {
                    value: interval.xmin,
                    context: TimeRole::IntervalStart,
                });
            }
            if !interval.xmax.is_finite() {
                issues.push(IntegrityIssue::NonFiniteTime {
                    value: interval.xmax,
                    context: TimeRole::IntervalEnd,
                });
            }
            if interval.xmin >= interval.xmax {
                issues.push(IntegrityIssue::IntervalUnsorted {
                    tier: tier_id,
                    previous: interval.xmin,
                    next: interval.xmax,
                });
            }
            if index > 0 {
                let previous = &tier.intervals[index - 1];
                if previous.xmax > interval.xmin {
                    issues.push(IntegrityIssue::IntervalOverlap {
                        tier: tier_id,
                        left: previous.id,
                        right: interval.id,
                        overlap_start: interval.xmin,
                        overlap_end: previous.xmax,
                    });
                } else if previous.xmax < interval.xmin {
                    issues.push(IntegrityIssue::IntervalGap {
                        tier: tier_id,
                        left: previous.id,
                        right: interval.id,
                        gap_start: previous.xmax,
                        gap_end: interval.xmin,
                    });
                }
                if previous.end_boundary != interval.start_boundary {
                    issues.push(IntegrityIssue::BoundaryContinuityMismatch {
                        tier: tier_id,
                        left: previous.id,
                        right: interval.id,
                    });
                }
                tier_boundaries.push(interval.start_boundary);
            }
        }

        let last = tier.intervals.last().expect("non-empty interval tier");
        if !same_time(last.xmax, self.xmax) {
            issues.push(IntegrityIssue::DomainEndMismatch {
                tier: tier_id,
                found: last.xmax,
                expected: self.xmax,
            });
        }
        tier_boundaries.push(last.end_boundary);

        for boundary in tier_boundaries {
            if !boundary_ids.insert(boundary) {
                issues.push(IntegrityIssue::DuplicateBoundaryId {
                    tier: tier_id,
                    boundary,
                });
            }
        }
    }

    fn validate_point_tier(
        &self,
        tier_id: TierId,
        tier: &PointTier,
        point_ids: &mut HashSet<PointId>,
        issues: &mut Vec<IntegrityIssue>,
    ) {
        for (index, point) in tier.points.iter().enumerate() {
            if !point_ids.insert(point.id) {
                issues.push(IntegrityIssue::DuplicatePointId {
                    tier: tier_id,
                    point: point.id,
                });
            }
            if !point.time.is_finite() {
                issues.push(IntegrityIssue::NonFiniteTime {
                    value: point.time,
                    context: TimeRole::Point,
                });
            }
            if point.time < self.xmin || point.time > self.xmax {
                issues.push(IntegrityIssue::PointOutsideDomain {
                    tier: tier_id,
                    point: point.id,
                    time: point.time,
                });
            }
            if index > 0 {
                let previous = &tier.points[index - 1];
                if previous.time > point.time {
                    issues.push(IntegrityIssue::PointUnsorted {
                        tier: tier_id,
                        previous: previous.id,
                        next: point.id,
                    });
                } else if same_time(previous.time, point.time) {
                    issues.push(IntegrityIssue::DuplicatePointTime {
                        tier: tier_id,
                        previous: previous.id,
                        next: point.id,
                        time: point.time,
                    });
                }
            }
        }
    }

    fn validate_aligned_relation(
        &self,
        tier: TierId,
        with: TierId,
        issues: &mut Vec<IntegrityIssue>,
    ) {
        let Some(left_slot) = self.tier(tier) else {
            return;
        };
        let Some(right_slot) = self.tier(with) else {
            issues.push(IntegrityIssue::MissingTierRelationTarget { tier, target: with });
            return;
        };
        let (Tier::Interval(left), Tier::Interval(right)) = (&left_slot.tier, &right_slot.tier)
        else {
            issues.push(IntegrityIssue::RelationTierKindMismatch {
                tier,
                target: with,
                relation: RelationKind::AlignedBoundaries,
            });
            return;
        };

        for (boundary, time) in left.boundaries() {
            if !right
                .boundaries()
                .any(|(_, other_time)| same_time(time, other_time))
            {
                issues.push(IntegrityIssue::AlignedBoundaryMissing {
                    tier,
                    target: with,
                    boundary,
                    time,
                });
            }
        }
        for (boundary, time) in right.boundaries() {
            if !left
                .boundaries()
                .any(|(_, other_time)| same_time(time, other_time))
            {
                issues.push(IntegrityIssue::AlignedBoundaryMissing {
                    tier: with,
                    target: tier,
                    boundary,
                    time,
                });
            }
        }
    }

    fn validate_child_relation(
        &self,
        tier: TierId,
        parent: TierId,
        issues: &mut Vec<IntegrityIssue>,
    ) {
        let Some(child_slot) = self.tier(tier) else {
            return;
        };
        let Some(parent_slot) = self.tier(parent) else {
            issues.push(IntegrityIssue::MissingTierRelationTarget {
                tier,
                target: parent,
            });
            return;
        };
        let Tier::Interval(parent_tier) = &parent_slot.tier else {
            issues.push(IntegrityIssue::RelationTierKindMismatch {
                tier,
                target: parent,
                relation: RelationKind::ChildOf,
            });
            return;
        };
        match &child_slot.tier {
            Tier::Interval(child_tier) => {
                for child in &child_tier.intervals {
                    if !parent_tier
                        .intervals
                        .iter()
                        .any(|parent| child.xmin >= parent.xmin && child.xmax <= parent.xmax)
                    {
                        issues.push(IntegrityIssue::OrphanChildInterval {
                            tier,
                            parent,
                            interval: child.id,
                            xmin: child.xmin,
                            xmax: child.xmax,
                        });
                    }
                }
            }
            Tier::Point(_) => {
                issues.push(IntegrityIssue::RelationTierKindMismatch {
                    tier,
                    target: parent,
                    relation: RelationKind::ChildOf,
                });
            }
        }
    }

    fn alloc_tier_id(&mut self) -> TierId {
        let id = TierId(self.next_tier_id);
        self.next_tier_id += 1;
        id
    }

    fn alloc_boundary_id(&mut self) -> BoundaryId {
        let id = BoundaryId(self.next_boundary_id);
        self.next_boundary_id += 1;
        id
    }

    fn alloc_interval_id(&mut self) -> IntervalId {
        let id = IntervalId(self.next_interval_id);
        self.next_interval_id += 1;
        id
    }

    fn alloc_point_id(&mut self) -> PointId {
        let id = PointId(self.next_point_id);
        self.next_point_id += 1;
        id
    }

    fn reseed_ids(&mut self) {
        self.next_tier_id = self.tiers.iter().map(|slot| slot.id.0).max().unwrap_or(0) + 1;
        self.next_boundary_id = self
            .tiers
            .iter()
            .filter_map(|slot| match &slot.tier {
                Tier::Interval(tier) => Some(tier.boundaries().map(|(id, _)| id.0)),
                Tier::Point(_) => None,
            })
            .flatten()
            .max()
            .unwrap_or(0)
            + 1;
        self.next_interval_id = self
            .tiers
            .iter()
            .filter_map(|slot| match &slot.tier {
                Tier::Interval(tier) => Some(tier.intervals.iter().map(|interval| interval.id.0)),
                Tier::Point(_) => None,
            })
            .flatten()
            .max()
            .unwrap_or(0)
            + 1;
        self.next_point_id = self
            .tiers
            .iter()
            .filter_map(|slot| match &slot.tier {
                Tier::Interval(_) => None,
                Tier::Point(tier) => Some(tier.points.iter().map(|point| point.id.0)),
            })
            .flatten()
            .max()
            .unwrap_or(0)
            + 1;
    }

    fn tier_index(&self, tier: TierId) -> Result<usize, AnnotationError> {
        self.tiers
            .iter()
            .position(|slot| slot.id == tier)
            .ok_or(AnnotationError::UnknownTier { tier })
    }

    fn commit_if_valid(&self) -> Result<(), AnnotationError> {
        match self.validate().into_iter().next() {
            Some(issue) => Err(AnnotationError::IntegrityViolation(issue)),
            None => Ok(()),
        }
    }

    fn aligned_interval_indices_for_tier(
        &self,
        tier: TierId,
    ) -> Result<Vec<usize>, AnnotationError> {
        let start_index = self.tier_index(tier)?;
        if !matches!(self.tiers[start_index].tier, Tier::Interval(_)) {
            return Err(AnnotationError::InvalidTierKind {
                tier,
                expected: TierKind::Interval,
            });
        }

        let mut included = HashSet::from([tier]);
        let mut changed = true;
        while changed {
            changed = false;
            for slot in &self.tiers {
                if let TierRelation::AlignedBoundaries { with } = slot.relation {
                    let connects = included.contains(&slot.id) || included.contains(&with);
                    if connects {
                        if self.tier(with).is_none() {
                            return Err(AnnotationError::UnknownTier { tier: with });
                        }
                        if included.insert(slot.id) {
                            changed = true;
                        }
                        if included.insert(with) {
                            changed = true;
                        }
                    }
                }
            }
        }

        let mut indices = Vec::new();
        for (index, slot) in self.tiers.iter().enumerate() {
            if included.contains(&slot.id) {
                if !matches!(slot.tier, Tier::Interval(_)) {
                    return Err(AnnotationError::InvalidTierKind {
                        tier: slot.id,
                        expected: TierKind::Interval,
                    });
                }
                indices.push(index);
            }
        }
        Ok(indices)
    }

    fn boundary_location(&self, id: BoundaryId) -> Result<(TierId, f64), AnnotationError> {
        for slot in &self.tiers {
            if let Tier::Interval(tier) = &slot.tier {
                for (boundary, time) in tier.boundaries() {
                    if boundary == id {
                        return Ok((slot.id, time));
                    }
                }
            }
        }
        Err(AnnotationError::UnknownBoundary { boundary: id })
    }

    fn split_interval_at_index(
        &mut self,
        tier_index: usize,
        at: f64,
    ) -> Result<BoundaryId, AnnotationError> {
        let new_boundary = self.alloc_boundary_id();
        let new_interval = self.alloc_interval_id();
        let tier_id = self.tiers[tier_index].id;
        let Tier::Interval(tier) = &mut self.tiers[tier_index].tier else {
            return Err(AnnotationError::InvalidTierKind {
                tier: tier_id,
                expected: TierKind::Interval,
            });
        };
        let Some(index) = tier
            .intervals
            .iter()
            .position(|interval| at > interval.xmin && at < interval.xmax)
        else {
            if tier
                .boundaries()
                .any(|(_, boundary_time)| same_time(boundary_time, at))
            {
                return Err(AnnotationError::BoundaryAlreadyExists { tier: tier_id, at });
            }
            return Err(AnnotationError::BoundaryOutsideInterval { tier: tier_id, at });
        };
        let original = tier.intervals[index].clone();
        tier.intervals[index] = Interval {
            id: original.id,
            start_boundary: original.start_boundary,
            end_boundary: new_boundary,
            xmin: original.xmin,
            xmax: at,
            label: original.label.clone(),
        };
        tier.intervals.insert(
            index + 1,
            Interval {
                id: new_interval,
                start_boundary: new_boundary,
                end_boundary: original.end_boundary,
                xmin: at,
                xmax: original.xmax,
                label: original.label,
            },
        );
        Ok(new_boundary)
    }

    fn move_boundary_in_tier_at(
        &mut self,
        tier_index: usize,
        at: f64,
        to: f64,
    ) -> Result<BoundaryMove, AnnotationError> {
        let tier_id = self.tiers[tier_index].id;
        let Tier::Interval(tier) = &mut self.tiers[tier_index].tier else {
            return Err(AnnotationError::InvalidTierKind {
                tier: tier_id,
                expected: TierKind::Interval,
            });
        };
        let Some(index) = tier.interior_boundary_index_at(at) else {
            return Err(AnnotationError::UnknownBoundaryAt { tier: tier_id, at });
        };
        let left_start = tier.intervals[index].xmin;
        let right_end = tier.intervals[index + 1].xmax;
        if to <= left_start || to >= right_end {
            return Err(AnnotationError::BoundaryWouldCollapseInterval {
                tier: tier_id,
                at: to,
            });
        }
        let boundary = tier.intervals[index].end_boundary;
        tier.intervals[index].xmax = to;
        tier.intervals[index + 1].xmin = to;
        Ok(BoundaryMove {
            tier: tier_id,
            boundary,
            from: at,
            to,
        })
    }

    fn merge_boundary_in_tier_at(
        &mut self,
        tier_index: usize,
        at: f64,
    ) -> Result<TierMerge, AnnotationError> {
        let tier_id = self.tiers[tier_index].id;
        let Tier::Interval(tier) = &mut self.tiers[tier_index].tier else {
            return Err(AnnotationError::InvalidTierKind {
                tier: tier_id,
                expected: TierKind::Interval,
            });
        };
        let Some(index) = tier.interior_boundary_index_at(at) else {
            return Err(AnnotationError::BoundaryAtDomainEdge { tier: tier_id, at });
        };
        let left = tier.intervals[index].clone();
        let right = tier.intervals[index + 1].clone();
        let merged = Interval {
            id: left.id,
            start_boundary: left.start_boundary,
            end_boundary: right.end_boundary,
            xmin: left.xmin,
            xmax: right.xmax,
            label: left.label.clone(),
        };
        tier.intervals.splice(index..=index + 1, [merged.clone()]);
        Ok(TierMerge {
            tier: tier_id,
            boundary: left.end_boundary,
            at,
            left,
            right,
            merged,
        })
    }

    fn restore_merge(&mut self, merge: &TierMerge) -> Result<(), AnnotationError> {
        let index = self.tier_index(merge.tier)?;
        let Tier::Interval(tier) = &mut self.tiers[index].tier else {
            return Err(AnnotationError::InvalidTierKind {
                tier: merge.tier,
                expected: TierKind::Interval,
            });
        };
        let Some(position) = tier
            .intervals
            .iter()
            .position(|interval| interval == &merge.merged)
        else {
            return Err(AnnotationError::MergedIntervalNotFound {
                tier: merge.tier,
                interval: merge.merged.id,
            });
        };
        tier.intervals.splice(
            position..=position,
            [merge.left.clone(), merge.right.clone()],
        );
        self.reseed_ids();
        Ok(())
    }

    fn replace_label(
        &mut self,
        target: LabelTarget,
        text: &str,
    ) -> Result<String, AnnotationError> {
        match target {
            LabelTarget::Interval { tier, interval } => {
                let index = self.tier_index(tier)?;
                let Tier::Interval(tier_data) = &mut self.tiers[index].tier else {
                    return Err(AnnotationError::InvalidTierKind {
                        tier,
                        expected: TierKind::Interval,
                    });
                };
                let Some(interval) = tier_data
                    .intervals
                    .iter_mut()
                    .find(|candidate| candidate.id == interval)
                else {
                    return Err(AnnotationError::UnknownInterval { interval });
                };
                let old = std::mem::replace(&mut interval.label, text.to_owned());
                Ok(old)
            }
            LabelTarget::Point { tier, point } => {
                let index = self.tier_index(tier)?;
                let Tier::Point(tier_data) = &mut self.tiers[index].tier else {
                    return Err(AnnotationError::InvalidTierKind {
                        tier,
                        expected: TierKind::Point,
                    });
                };
                let Some(point_data) = tier_data
                    .points
                    .iter_mut()
                    .find(|candidate| candidate.id == point)
                else {
                    return Err(AnnotationError::UnknownPoint { point });
                };
                let old = std::mem::replace(&mut point_data.label, text.to_owned());
                Ok(old)
            }
        }
    }

    fn insert_point_into_tier(
        &mut self,
        tier: TierId,
        point: Point,
    ) -> Result<Point, AnnotationError> {
        let index = self.tier_index(tier)?;
        let tier_id = self.tiers[index].id;
        let Tier::Point(tier_data) = &mut self.tiers[index].tier else {
            return Err(AnnotationError::InvalidTierKind {
                tier: tier_id,
                expected: TierKind::Point,
            });
        };
        let position = tier_data
            .points
            .partition_point(|existing| existing.time < point.time);
        tier_data.points.insert(position, point.clone());
        Ok(point)
    }

    fn move_point_time(&mut self, id: PointId, to: f64) -> Result<(TierId, f64), AnnotationError> {
        for slot in &mut self.tiers {
            if let Tier::Point(tier_data) = &mut slot.tier
                && let Some(point) = tier_data.points.iter_mut().find(|point| point.id == id)
            {
                let from = point.time;
                point.time = to;
                return Ok((slot.id, from));
            }
        }
        Err(AnnotationError::UnknownPoint { point: id })
    }

    fn remove_point_by_id(&mut self, id: PointId) -> Result<(TierId, Point), AnnotationError> {
        for slot in &mut self.tiers {
            if let Tier::Point(tier_data) = &mut slot.tier
                && let Some(position) = tier_data.points.iter().position(|point| point.id == id)
            {
                let removed = tier_data.points.remove(position);
                return Ok((slot.id, removed));
            }
        }
        Err(AnnotationError::UnknownPoint { point: id })
    }
}

/// Ordered tier entry carrying the tier identifier and relation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TierSlot {
    /// Stable tier identifier.
    pub id: TierId,
    /// Relation between this tier and another tier.
    pub relation: TierRelation,
    /// Tier payload.
    pub tier: Tier,
}

/// Annotation tier variant.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Tier {
    /// Interval tier with contiguous spans.
    Interval(IntervalTier),
    /// Point tier with strictly increasing point times.
    Point(PointTier),
}

/// Interval tier with sorted, contiguous intervals over the document domain.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IntervalTier {
    /// Display name for the tier.
    pub name: String,
    /// Ordered intervals.
    pub intervals: Vec<Interval>,
}

impl IntervalTier {
    fn boundaries(&self) -> impl Iterator<Item = (BoundaryId, f64)> + '_ {
        self.intervals
            .first()
            .map(|interval| (interval.start_boundary, interval.xmin))
            .into_iter()
            .chain(
                self.intervals
                    .iter()
                    .map(|interval| (interval.end_boundary, interval.xmax)),
            )
    }

    fn interior_boundary_index_at(&self, at: f64) -> Option<usize> {
        self.intervals
            .windows(2)
            .position(|pair| same_time(pair[0].xmax, at) && same_time(pair[1].xmin, at))
    }
}

/// Labeled interval bounded by two stable boundary identifiers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Interval {
    /// Stable interval identifier.
    pub id: IntervalId,
    /// Stable identifier for the interval start boundary.
    pub start_boundary: BoundaryId,
    /// Stable identifier for the interval end boundary.
    pub end_boundary: BoundaryId,
    /// Start time in seconds.
    pub xmin: f64,
    /// End time in seconds.
    pub xmax: f64,
    /// Label text; an empty string represents an unlabeled interval.
    pub label: String,
}

/// Point tier with sorted, strictly increasing points.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PointTier {
    /// Display name for the tier.
    pub name: String,
    /// Ordered point annotations.
    pub points: Vec<Point>,
}

/// Labeled point annotation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Point {
    /// Stable point identifier.
    pub id: PointId,
    /// Point time in seconds.
    pub time: f64,
    /// Label text.
    pub label: String,
}

/// Relationship between a tier and another tier.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum TierRelation {
    /// The tier has no cross-tier integrity relation.
    Independent,
    /// The tier shares interval boundaries with another interval tier.
    AlignedBoundaries {
        /// Related tier whose boundary times must match this tier.
        with: TierId,
    },
    /// The tier's intervals must be nested inside intervals on a parent tier.
    ChildOf {
        /// Parent interval tier.
        parent: TierId,
    },
}

/// Boundary movement mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum AlignMode {
    /// Move all boundaries at the same time in the aligned tier component.
    Linked,
    /// Move one tier only; this fails when the boundary belongs to an aligned component.
    SingleTier,
}

/// Label target for interval and point labels.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum LabelTarget {
    /// Label of an interval in a tier.
    Interval {
        /// Tier containing the interval.
        tier: TierId,
        /// Interval whose label is addressed.
        interval: IntervalId,
    },
    /// Label of a point in a tier.
    Point {
        /// Tier containing the point.
        tier: TierId,
        /// Point whose label is addressed.
        point: PointId,
    },
}

/// Search query for interval and point labels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LabelQuery {
    /// Pattern used to match label text.
    pub pattern: LabelPattern,
    /// Optional tier filter; `None` searches all tiers.
    pub tiers: Option<Vec<TierId>>,
}

impl LabelQuery {
    /// Creates a substring query over all tiers.
    pub fn substring(text: &str) -> Self {
        Self {
            pattern: LabelPattern::Substring(text.to_owned()),
            tiers: None,
        }
    }

    /// Creates a regular expression query over all tiers.
    ///
    /// Invalid regular expression syntax produces no hits when passed to `search`.
    pub fn regex(pattern: &str) -> Self {
        Self {
            pattern: LabelPattern::Regex(pattern.to_owned()),
            tiers: None,
        }
    }

    /// Returns this query restricted to the supplied tiers.
    pub fn in_tiers(mut self, tiers: Vec<TierId>) -> Self {
        self.tiers = Some(tiers);
        self
    }

    fn matcher(&self) -> Option<Matcher> {
        match &self.pattern {
            LabelPattern::Substring(text) => Some(Matcher::Substring(text.clone())),
            LabelPattern::Regex(pattern) => Regex::new(pattern).ok().map(Matcher::Regex),
        }
    }
}

/// Label search pattern.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum LabelPattern {
    /// Literal substring search.
    Substring(String),
    /// Rust `regex` crate pattern.
    Regex(String),
}

enum Matcher {
    Substring(String),
    Regex(Regex),
}

impl Matcher {
    fn find_all(&self, text: &str) -> Vec<MatchSpan> {
        match self {
            Self::Substring(needle) => {
                if needle.is_empty() {
                    return Vec::new();
                }
                text.match_indices(needle)
                    .map(|(start, found)| MatchSpan {
                        start,
                        end: start + found.len(),
                    })
                    .collect()
            }
            Self::Regex(regex) => regex
                .find_iter(text)
                .map(|found| MatchSpan {
                    start: found.start(),
                    end: found.end(),
                })
                .collect(),
        }
    }
}

/// Search hit with tier, label target, and byte match span.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Hit {
    /// Tier containing the matched label.
    pub tier: TierId,
    /// Label target containing the matched text.
    pub target: LabelTarget,
    /// Byte span of the match within the label string.
    pub span: MatchSpan,
}

/// Byte span for a search match inside a UTF-8 label string.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MatchSpan {
    /// Inclusive start byte offset.
    pub start: usize,
    /// Exclusive end byte offset.
    pub end: usize,
}

/// Successful boundary movement payload.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Moved {
    /// Boundary movements performed by the call.
    pub moves: Vec<BoundaryMove>,
}

impl Moved {
    /// Builds the inverse operation for a successful `Annotation::move_boundary` call.
    ///
    /// The original call's `id`, `to`, and `mode` arguments are represented by
    /// the stored movement set, including aligned peer boundaries.
    pub fn inverse(&self) -> InverseMutation {
        InverseMutation::MoveBoundaries {
            moves: self.moves.clone(),
        }
    }
}

/// One boundary movement inside a single interval tier.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BoundaryMove {
    /// Tier containing the boundary.
    pub tier: TierId,
    /// Boundary that moved.
    pub boundary: BoundaryId,
    /// Previous boundary time in seconds.
    pub from: f64,
    /// New boundary time in seconds.
    pub to: f64,
}

/// Successful boundary removal payload.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Merged {
    /// Per-tier merges performed by the call.
    pub merges: Vec<TierMerge>,
}

impl Merged {
    /// Builds the inverse operation for a successful `Annotation::remove_boundary` call.
    ///
    /// The payload stores both original intervals, labels, identifiers, and the
    /// removed boundary, so applying the inverse restores the exact tier content.
    pub fn inverse(&self) -> InverseMutation {
        InverseMutation::RestoreMergedBoundary {
            merged: self.clone(),
        }
    }
}

/// Merge performed within one interval tier.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TierMerge {
    /// Tier where the merge occurred.
    pub tier: TierId,
    /// Boundary removed by the merge.
    pub boundary: BoundaryId,
    /// Removed boundary time in seconds.
    pub at: f64,
    /// Left interval before the merge.
    pub left: Interval,
    /// Right interval before the merge.
    pub right: Interval,
    /// Merged interval after the boundary was removed.
    pub merged: Interval,
}

/// Successful label replacement payload.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LabelChange {
    /// Label target that was changed.
    pub target: LabelTarget,
    /// Previous label text.
    pub old_text: String,
    /// New label text.
    pub new_text: String,
}

impl LabelChange {
    /// Builds the inverse operation for a successful `Annotation::set_label` call.
    ///
    /// The original `target` and `text` arguments are represented by this
    /// payload; applying the inverse writes the previous label text back.
    pub fn inverse(&self) -> InverseMutation {
        InverseMutation::SetLabel {
            target: self.target,
            text: self.old_text.clone(),
        }
    }
}

/// Successful point insertion payload.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PointInsertion {
    /// Tier that received the point.
    pub tier: TierId,
    /// Inserted point, carrying its allocated identifier.
    pub point: Point,
}

impl PointInsertion {
    /// Builds the inverse operation for a successful `Annotation::insert_point` call.
    ///
    /// Applying the inverse removes the inserted point by its allocated id.
    pub fn inverse(&self) -> InverseMutation {
        InverseMutation::RemovePoint {
            point: self.point.id,
        }
    }
}

/// Successful point movement payload.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PointMoved {
    /// Tier containing the point.
    pub tier: TierId,
    /// Point that moved.
    pub point: PointId,
    /// Previous point time in seconds.
    pub from: f64,
    /// New point time in seconds.
    pub to: f64,
}

impl PointMoved {
    /// Builds the inverse operation for a successful `Annotation::move_point` call.
    ///
    /// Applying the inverse returns the point to its previous time.
    pub fn inverse(&self) -> InverseMutation {
        InverseMutation::MovePoint {
            point: self.point,
            to: self.from,
        }
    }
}

/// Successful point removal payload.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PointRemoval {
    /// Tier that held the point.
    pub tier: TierId,
    /// Removed point, carrying its identifier, time, and label.
    pub point: Point,
}

impl PointRemoval {
    /// Builds the inverse operation for a successful `Annotation::remove_point` call.
    ///
    /// Applying the inverse restores the exact point at its time-sorted position.
    pub fn inverse(&self) -> InverseMutation {
        InverseMutation::RestorePoint {
            tier: self.tier,
            point: self.point.clone(),
        }
    }
}

/// Successful tier reorder payload.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TierReorder {
    /// Tier that moved.
    pub tier: TierId,
    /// Index the tier occupied before the move.
    pub from_index: usize,
    /// Index the tier occupies after the move.
    pub to_index: usize,
}

impl TierReorder {
    /// Builds the inverse operation for a successful `Annotation::reorder_tier` call.
    ///
    /// Applying the inverse moves the tier back to the index it left.
    pub fn inverse(&self) -> InverseMutation {
        InverseMutation::ReorderTier {
            tier: self.tier,
            to_index: self.from_index,
        }
    }
}

/// Stored inverse mutation that can undo a successful mutator call.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum InverseMutation {
    /// Undo an insertion by removing the inserted boundary.
    RemoveBoundary {
        /// Boundary returned by `Annotation::insert_boundary`.
        boundary: BoundaryId,
    },
    /// Undo a movement by restoring every listed boundary to its previous time.
    MoveBoundaries {
        /// Boundary movement records returned by `Annotation::move_boundary`.
        moves: Vec<BoundaryMove>,
    },
    /// Undo a merge by restoring the removed boundary and both original intervals.
    RestoreMergedBoundary {
        /// Merge payload returned by `Annotation::remove_boundary`.
        merged: Merged,
    },
    /// Undo a label replacement by writing the previous label text.
    SetLabel {
        /// Label target.
        target: LabelTarget,
        /// Label text to restore.
        text: String,
    },
    /// Undo a point insertion by removing the inserted point.
    RemovePoint {
        /// Point returned by `Annotation::insert_point`.
        point: PointId,
    },
    /// Undo a point removal by restoring the exact point at its sorted position.
    RestorePoint {
        /// Tier that held the point.
        tier: TierId,
        /// Point to restore, with its original identifier, time, and label.
        point: Point,
    },
    /// Undo a point movement by returning the point to its previous time.
    MovePoint {
        /// Point to move.
        point: PointId,
        /// Time to restore.
        to: f64,
    },
    /// Undo a tier reorder by moving the tier back to its previous index.
    ReorderTier {
        /// Tier to move.
        tier: TierId,
        /// Index to restore.
        to_index: usize,
    },
}

/// Expected tier kind for validation and mutation errors.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum TierKind {
    /// Interval tier.
    Interval,
    /// Point tier.
    Point,
}

/// Relation kind used in integrity reports.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RelationKind {
    /// Shared-boundary relation.
    AlignedBoundaries,
    /// Parent-child nesting relation.
    ChildOf,
}

/// Role of a time value in an annotation document.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum TimeRole {
    /// Document domain start.
    DomainStart,
    /// Document domain end.
    DomainEnd,
    /// Boundary time.
    Boundary,
    /// Interval start.
    IntervalStart,
    /// Interval end.
    IntervalEnd,
    /// Point time.
    Point,
}

/// Integrity issue found during document validation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum IntegrityIssue {
    /// Document time domain is empty or reversed.
    InvalidTimeDomain {
        /// Domain start.
        xmin: f64,
        /// Domain end.
        xmax: f64,
    },
    /// A time value is not finite.
    NonFiniteTime {
        /// Invalid time value.
        value: f64,
        /// Role of the invalid value.
        context: TimeRole,
    },
    /// A tier identifier appears more than once.
    DuplicateTierId {
        /// Duplicated tier.
        tier: TierId,
    },
    /// An interval identifier appears more than once.
    DuplicateIntervalId {
        /// Tier containing the duplicate.
        tier: TierId,
        /// Duplicated interval.
        interval: IntervalId,
    },
    /// A point identifier appears more than once.
    DuplicatePointId {
        /// Tier containing the duplicate.
        tier: TierId,
        /// Duplicated point.
        point: PointId,
    },
    /// A boundary identifier appears in more than one logical boundary position.
    DuplicateBoundaryId {
        /// Tier where the duplicate was observed.
        tier: TierId,
        /// Duplicated boundary.
        boundary: BoundaryId,
    },
    /// Interval tier contains no intervals.
    IntervalTierEmpty {
        /// Empty tier.
        tier: TierId,
    },
    /// First interval does not begin at the document start.
    DomainStartMismatch {
        /// Tier with the mismatch.
        tier: TierId,
        /// Found start time.
        found: f64,
        /// Expected document start.
        expected: f64,
    },
    /// Last interval does not end at the document end.
    DomainEndMismatch {
        /// Tier with the mismatch.
        tier: TierId,
        /// Found end time.
        found: f64,
        /// Expected document end.
        expected: f64,
    },
    /// Interval boundaries are reversed or equal.
    IntervalUnsorted {
        /// Tier containing the interval.
        tier: TierId,
        /// Earlier boundary value.
        previous: f64,
        /// Later boundary value.
        next: f64,
    },
    /// Adjacent intervals overlap.
    IntervalOverlap {
        /// Tier containing the overlap.
        tier: TierId,
        /// Left interval.
        left: IntervalId,
        /// Right interval.
        right: IntervalId,
        /// Overlap start.
        overlap_start: f64,
        /// Overlap end.
        overlap_end: f64,
    },
    /// Adjacent intervals leave an uncovered gap.
    IntervalGap {
        /// Tier containing the gap.
        tier: TierId,
        /// Left interval.
        left: IntervalId,
        /// Right interval.
        right: IntervalId,
        /// Gap start.
        gap_start: f64,
        /// Gap end.
        gap_end: f64,
    },
    /// Adjacent intervals do not share the same boundary identifier.
    BoundaryContinuityMismatch {
        /// Tier containing the mismatch.
        tier: TierId,
        /// Left interval.
        left: IntervalId,
        /// Right interval.
        right: IntervalId,
    },
    /// Point time is outside the document domain.
    PointOutsideDomain {
        /// Tier containing the point.
        tier: TierId,
        /// Point identifier.
        point: PointId,
        /// Point time.
        time: f64,
    },
    /// Point times are out of order.
    PointUnsorted {
        /// Tier containing the points.
        tier: TierId,
        /// Previous point.
        previous: PointId,
        /// Next point.
        next: PointId,
    },
    /// Two adjacent points have the same time.
    DuplicatePointTime {
        /// Tier containing the points.
        tier: TierId,
        /// Previous point.
        previous: PointId,
        /// Next point.
        next: PointId,
        /// Duplicated time.
        time: f64,
    },
    /// Tier relation points to a missing tier.
    MissingTierRelationTarget {
        /// Tier carrying the relation.
        tier: TierId,
        /// Missing relation target.
        target: TierId,
    },
    /// Tier relation is attached to an unsupported tier kind.
    RelationTierKindMismatch {
        /// Tier carrying the relation.
        tier: TierId,
        /// Relation target.
        target: TierId,
        /// Relation kind.
        relation: RelationKind,
    },
    /// An aligned boundary is missing from the related tier.
    AlignedBoundaryMissing {
        /// Tier containing the boundary.
        tier: TierId,
        /// Related tier.
        target: TierId,
        /// Boundary missing from the related tier.
        boundary: BoundaryId,
        /// Boundary time.
        time: f64,
    },
    /// A child interval is not nested inside any parent interval.
    OrphanChildInterval {
        /// Child tier.
        tier: TierId,
        /// Parent tier.
        parent: TierId,
        /// Child interval.
        interval: IntervalId,
        /// Child start time.
        xmin: f64,
        /// Child end time.
        xmax: f64,
    },
}

impl fmt::Display for IntegrityIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTimeDomain { xmin, xmax } => {
                write!(f, "invalid annotation time domain [{xmin}, {xmax}]")
            }
            Self::NonFiniteTime { value, context } => {
                write!(f, "non-finite {context:?} time: {value}")
            }
            Self::DuplicateTierId { tier } => write!(f, "duplicate tier id {}", tier.get()),
            Self::DuplicateIntervalId { tier, interval } => write!(
                f,
                "duplicate interval id {} in tier {}",
                interval.get(),
                tier.get()
            ),
            Self::DuplicatePointId { tier, point } => {
                write!(
                    f,
                    "duplicate point id {} in tier {}",
                    point.get(),
                    tier.get()
                )
            }
            Self::DuplicateBoundaryId { tier, boundary } => write!(
                f,
                "duplicate boundary id {} in tier {}",
                boundary.get(),
                tier.get()
            ),
            Self::IntervalTierEmpty { tier } => {
                write!(f, "interval tier {} contains no intervals", tier.get())
            }
            Self::DomainStartMismatch {
                tier,
                found,
                expected,
            } => write!(
                f,
                "tier {} starts at {found}, expected {expected}",
                tier.get()
            ),
            Self::DomainEndMismatch {
                tier,
                found,
                expected,
            } => write!(
                f,
                "tier {} ends at {found}, expected {expected}",
                tier.get()
            ),
            Self::IntervalUnsorted {
                tier,
                previous,
                next,
            } => write!(
                f,
                "interval boundary order violation in tier {}: {previous} then {next}",
                tier.get()
            ),
            Self::IntervalOverlap {
                tier, left, right, ..
            } => write!(
                f,
                "intervals {} and {} overlap in tier {}",
                left.get(),
                right.get(),
                tier.get()
            ),
            Self::IntervalGap {
                tier, left, right, ..
            } => write!(
                f,
                "intervals {} and {} leave a gap in tier {}",
                left.get(),
                right.get(),
                tier.get()
            ),
            Self::BoundaryContinuityMismatch { tier, left, right } => write!(
                f,
                "intervals {} and {} do not share a boundary id in tier {}",
                left.get(),
                right.get(),
                tier.get()
            ),
            Self::PointOutsideDomain { tier, point, time } => write!(
                f,
                "point {} in tier {} is outside the document domain at {time}",
                point.get(),
                tier.get()
            ),
            Self::PointUnsorted {
                tier,
                previous,
                next,
            } => write!(
                f,
                "points {} and {} are out of order in tier {}",
                previous.get(),
                next.get(),
                tier.get()
            ),
            Self::DuplicatePointTime {
                tier,
                previous,
                next,
                time,
            } => write!(
                f,
                "points {} and {} share time {time} in tier {}",
                previous.get(),
                next.get(),
                tier.get()
            ),
            Self::MissingTierRelationTarget { tier, target } => write!(
                f,
                "tier {} relation references missing tier {}",
                tier.get(),
                target.get()
            ),
            Self::RelationTierKindMismatch {
                tier,
                target,
                relation,
            } => write!(
                f,
                "tier {} relation {:?} is incompatible with target {}",
                tier.get(),
                relation,
                target.get()
            ),
            Self::AlignedBoundaryMissing {
                tier,
                target,
                boundary,
                time,
            } => write!(
                f,
                "boundary {} at {time} in tier {} is missing from aligned tier {}",
                boundary.get(),
                tier.get(),
                target.get()
            ),
            Self::OrphanChildInterval {
                tier,
                parent,
                interval,
                ..
            } => write!(
                f,
                "child interval {} in tier {} is not nested in parent tier {}",
                interval.get(),
                tier.get(),
                parent.get()
            ),
        }
    }
}

impl std::error::Error for IntegrityIssue {}

/// Errors produced by annotation mutation and validation guards.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum AnnotationError {
    /// Document time domain is empty or reversed.
    InvalidTimeDomain {
        /// Domain start.
        xmin: f64,
        /// Domain end.
        xmax: f64,
    },
    /// A time value is not finite.
    NonFiniteTime {
        /// Invalid time value.
        value: f64,
        /// Role of the invalid value.
        context: TimeRole,
    },
    /// Tier identifier is unknown.
    UnknownTier {
        /// Missing tier.
        tier: TierId,
    },
    /// Boundary identifier is unknown.
    UnknownBoundary {
        /// Missing boundary.
        boundary: BoundaryId,
    },
    /// No boundary exists at the requested time in the tier.
    UnknownBoundaryAt {
        /// Tier searched for the boundary.
        tier: TierId,
        /// Boundary time.
        at: f64,
    },
    /// Interval identifier is unknown.
    UnknownInterval {
        /// Missing interval.
        interval: IntervalId,
    },
    /// Point identifier is unknown.
    UnknownPoint {
        /// Missing point.
        point: PointId,
    },
    /// Tier has the wrong kind for the requested operation.
    InvalidTierKind {
        /// Tier with the wrong kind.
        tier: TierId,
        /// Expected tier kind.
        expected: TierKind,
    },
    /// Boundary insertion attempted to reuse an existing boundary time.
    BoundaryAlreadyExists {
        /// Tier containing the existing boundary.
        tier: TierId,
        /// Boundary time.
        at: f64,
    },
    /// Boundary insertion time is outside every interval.
    BoundaryOutsideInterval {
        /// Tier searched for an interval.
        tier: TierId,
        /// Requested boundary time.
        at: f64,
    },
    /// Boundary movement would collapse an adjacent interval.
    BoundaryWouldCollapseInterval {
        /// Tier containing the boundary.
        tier: TierId,
        /// Requested boundary time.
        at: f64,
    },
    /// Boundary removal attempted to remove a domain edge.
    BoundaryAtDomainEdge {
        /// Tier containing the boundary.
        tier: TierId,
        /// Requested boundary time.
        at: f64,
    },
    /// An aligned boundary was moved with `AlignMode::SingleTier`.
    AlignedBoundaryRequiresLinkedMode {
        /// Boundary that belongs to an aligned component.
        boundary: BoundaryId,
    },
    /// Label text contains a rejected control character.
    InvalidLabelControl {
        /// Rejected Unicode scalar value.
        codepoint: u32,
        /// Byte index where the character starts.
        index: usize,
    },
    /// A candidate mutation would leave the document invalid.
    IntegrityViolation(IntegrityIssue),
    /// Merged interval stored in an inverse operation was not found.
    MergedIntervalNotFound {
        /// Tier searched for the merged interval.
        tier: TierId,
        /// Merged interval identifier.
        interval: IntervalId,
    },
}

impl fmt::Display for AnnotationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTimeDomain { xmin, xmax } => {
                write!(f, "invalid annotation time domain [{xmin}, {xmax}]")
            }
            Self::NonFiniteTime { value, context } => {
                write!(f, "non-finite {context:?} time: {value}")
            }
            Self::UnknownTier { tier } => write!(f, "unknown tier id {}", tier.get()),
            Self::UnknownBoundary { boundary } => {
                write!(f, "unknown boundary id {}", boundary.get())
            }
            Self::UnknownBoundaryAt { tier, at } => {
                write!(f, "no boundary at {at} in tier {}", tier.get())
            }
            Self::UnknownInterval { interval } => {
                write!(f, "unknown interval id {}", interval.get())
            }
            Self::UnknownPoint { point } => write!(f, "unknown point id {}", point.get()),
            Self::InvalidTierKind { tier, expected } => {
                write!(f, "tier {} is not a {expected:?} tier", tier.get())
            }
            Self::BoundaryAlreadyExists { tier, at } => {
                write!(f, "tier {} already contains a boundary at {at}", tier.get())
            }
            Self::BoundaryOutsideInterval { tier, at } => write!(
                f,
                "boundary time {at} is outside every interval in tier {}",
                tier.get()
            ),
            Self::BoundaryWouldCollapseInterval { tier, at } => write!(
                f,
                "moving boundary to {at} would collapse an interval in tier {}",
                tier.get()
            ),
            Self::BoundaryAtDomainEdge { tier, at } => {
                write!(
                    f,
                    "boundary at {at} is a domain edge in tier {}",
                    tier.get()
                )
            }
            Self::AlignedBoundaryRequiresLinkedMode { boundary } => write!(
                f,
                "boundary {} belongs to an aligned component",
                boundary.get()
            ),
            Self::InvalidLabelControl { codepoint, index } => write!(
                f,
                "label contains rejected control character U+{codepoint:04X} at byte {index}"
            ),
            Self::IntegrityViolation(issue) => write!(f, "annotation integrity violation: {issue}"),
            Self::MergedIntervalNotFound { tier, interval } => write!(
                f,
                "merged interval {} was not found in tier {}",
                interval.get(),
                tier.get()
            ),
        }
    }
}

impl std::error::Error for AnnotationError {}

fn reject_control_label(text: &str) -> Result<(), AnnotationError> {
    for (index, ch) in text.char_indices() {
        if ch == '\u{7f}' || (ch <= '\u{1f}') {
            return Err(AnnotationError::InvalidLabelControl {
                codepoint: ch as u32,
                index,
            });
        }
    }
    Ok(())
}

fn same_time(left: f64, right: f64) -> bool {
    left.to_bits() == right.to_bits()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Public mutators validate candidate states before commit; these raw cases
    // model malformed importer input that `validate` must report.
    #[test]
    fn validation_reports_invalid_time_domain() {
        let doc = Annotation::from_raw(1.0, 0.0, Vec::new());
        assert_single_issue(doc.validate(), |issue| {
            matches!(issue, IntegrityIssue::InvalidTimeDomain { .. })
        });
    }

    #[test]
    fn validation_reports_non_finite_time() {
        let doc = Annotation::from_raw(
            0.0,
            1.0,
            vec![TierSlot {
                id: TierId(1),
                relation: TierRelation::Independent,
                tier: Tier::Point(PointTier {
                    name: "points".to_owned(),
                    points: vec![Point {
                        id: PointId(1),
                        time: f64::NAN,
                        label: String::new(),
                    }],
                }),
            }],
        );
        assert_single_issue(doc.validate(), |issue| {
            matches!(issue, IntegrityIssue::NonFiniteTime { .. })
        });
    }

    #[test]
    fn validation_reports_duplicate_tier_id() {
        let doc = Annotation::from_raw(
            0.0,
            1.0,
            vec![empty_point_slot(1, "left"), empty_point_slot(1, "right")],
        );
        assert_single_issue(doc.validate(), |issue| {
            matches!(issue, IntegrityIssue::DuplicateTierId { .. })
        });
    }

    #[test]
    fn validation_reports_duplicate_interval_id() {
        let doc = raw_interval_doc(vec![(1, 1, 2, 0.0, 0.5), (1, 2, 3, 0.5, 1.0)]);
        assert_single_issue(doc.validate(), |issue| {
            matches!(issue, IntegrityIssue::DuplicateIntervalId { .. })
        });
    }

    #[test]
    fn validation_reports_duplicate_point_id() {
        let doc = Annotation::from_raw(
            0.0,
            1.0,
            vec![TierSlot {
                id: TierId(1),
                relation: TierRelation::Independent,
                tier: Tier::Point(PointTier {
                    name: "points".to_owned(),
                    points: vec![
                        Point {
                            id: PointId(1),
                            time: 0.2,
                            label: String::new(),
                        },
                        Point {
                            id: PointId(1),
                            time: 0.8,
                            label: String::new(),
                        },
                    ],
                }),
            }],
        );
        assert_single_issue(doc.validate(), |issue| {
            matches!(issue, IntegrityIssue::DuplicatePointId { .. })
        });
    }

    #[test]
    fn validation_reports_duplicate_boundary_id() {
        let doc = raw_interval_doc(vec![(1, 1, 2, 0.0, 0.5), (2, 2, 1, 0.5, 1.0)]);
        assert_single_issue(doc.validate(), |issue| {
            matches!(issue, IntegrityIssue::DuplicateBoundaryId { .. })
        });
    }

    #[test]
    fn validation_reports_empty_interval_tier() {
        let doc = Annotation::from_raw(
            0.0,
            1.0,
            vec![TierSlot {
                id: TierId(1),
                relation: TierRelation::Independent,
                tier: Tier::Interval(IntervalTier {
                    name: "empty".to_owned(),
                    intervals: Vec::new(),
                }),
            }],
        );
        assert_single_issue(doc.validate(), |issue| {
            matches!(issue, IntegrityIssue::IntervalTierEmpty { .. })
        });
    }

    #[test]
    fn validation_reports_domain_start_mismatch() {
        let doc = raw_interval_doc(vec![(1, 1, 2, 0.1, 1.0)]);
        assert_single_issue(doc.validate(), |issue| {
            matches!(issue, IntegrityIssue::DomainStartMismatch { .. })
        });
    }

    #[test]
    fn validation_reports_domain_end_mismatch() {
        let doc = raw_interval_doc(vec![(1, 1, 2, 0.0, 0.9)]);
        assert_single_issue(doc.validate(), |issue| {
            matches!(issue, IntegrityIssue::DomainEndMismatch { .. })
        });
    }

    #[test]
    fn validation_reports_interval_unsorted() {
        let doc = Annotation::from_raw(
            0.0,
            0.6,
            vec![TierSlot {
                id: TierId(1),
                relation: TierRelation::Independent,
                tier: Tier::Interval(IntervalTier {
                    name: "raw".to_owned(),
                    intervals: vec![interval(1, 1, 2, 0.0, 0.7), interval(2, 2, 3, 0.7, 0.6)],
                }),
            }],
        );
        assert_single_issue(doc.validate(), |issue| {
            matches!(issue, IntegrityIssue::IntervalUnsorted { .. })
        });
    }

    #[test]
    fn validation_reports_interval_overlap() {
        let doc = raw_interval_doc(vec![(1, 1, 2, 0.0, 0.7), (2, 2, 3, 0.5, 1.0)]);
        assert_single_issue(doc.validate(), |issue| {
            matches!(issue, IntegrityIssue::IntervalOverlap { .. })
        });
    }

    #[test]
    fn validation_reports_interval_gap() {
        let doc = raw_interval_doc(vec![(1, 1, 2, 0.0, 0.4), (2, 2, 3, 0.6, 1.0)]);
        assert_single_issue(doc.validate(), |issue| {
            matches!(issue, IntegrityIssue::IntervalGap { .. })
        });
    }

    #[test]
    fn validation_reports_boundary_continuity_mismatch() {
        let doc = raw_interval_doc(vec![(1, 1, 2, 0.0, 0.5), (2, 3, 4, 0.5, 1.0)]);
        assert_single_issue(doc.validate(), |issue| {
            matches!(issue, IntegrityIssue::BoundaryContinuityMismatch { .. })
        });
    }

    #[test]
    fn validation_reports_point_outside_domain() {
        let doc = Annotation::from_raw(
            0.0,
            1.0,
            vec![TierSlot {
                id: TierId(1),
                relation: TierRelation::Independent,
                tier: Tier::Point(PointTier {
                    name: "points".to_owned(),
                    points: vec![Point {
                        id: PointId(1),
                        time: 1.2,
                        label: String::new(),
                    }],
                }),
            }],
        );
        assert_single_issue(doc.validate(), |issue| {
            matches!(issue, IntegrityIssue::PointOutsideDomain { .. })
        });
    }

    #[test]
    fn validation_reports_unsorted_point_tier() {
        let doc = Annotation::from_raw(
            0.0,
            1.0,
            vec![TierSlot {
                id: TierId(1),
                relation: TierRelation::Independent,
                tier: Tier::Point(PointTier {
                    name: "points".to_owned(),
                    points: vec![
                        Point {
                            id: PointId(1),
                            time: 0.8,
                            label: String::new(),
                        },
                        Point {
                            id: PointId(2),
                            time: 0.2,
                            label: String::new(),
                        },
                    ],
                }),
            }],
        );
        assert_single_issue(doc.validate(), |issue| {
            matches!(issue, IntegrityIssue::PointUnsorted { .. })
        });
    }

    #[test]
    fn validation_reports_duplicate_point_time() {
        let doc = Annotation::from_raw(
            0.0,
            1.0,
            vec![TierSlot {
                id: TierId(1),
                relation: TierRelation::Independent,
                tier: Tier::Point(PointTier {
                    name: "points".to_owned(),
                    points: vec![
                        Point {
                            id: PointId(1),
                            time: 0.2,
                            label: String::new(),
                        },
                        Point {
                            id: PointId(2),
                            time: 0.2,
                            label: String::new(),
                        },
                    ],
                }),
            }],
        );
        assert_single_issue(doc.validate(), |issue| {
            matches!(issue, IntegrityIssue::DuplicatePointTime { .. })
        });
    }

    #[test]
    fn validation_reports_relation_target_missing() {
        let doc = Annotation::from_raw(
            0.0,
            1.0,
            vec![TierSlot {
                id: TierId(1),
                relation: TierRelation::ChildOf {
                    parent: TierId(999),
                },
                tier: Tier::Interval(IntervalTier {
                    name: "child".to_owned(),
                    intervals: vec![interval(1, 1, 2, 0.0, 1.0)],
                }),
            }],
        );
        assert_single_issue(doc.validate(), |issue| {
            matches!(issue, IntegrityIssue::MissingTierRelationTarget { .. })
        });
    }

    #[test]
    fn validation_reports_relation_kind_mismatch() {
        let doc = Annotation::from_raw(
            0.0,
            1.0,
            vec![
                TierSlot {
                    id: TierId(1),
                    relation: TierRelation::Independent,
                    tier: Tier::Interval(IntervalTier {
                        name: "parent".to_owned(),
                        intervals: vec![interval(1, 1, 2, 0.0, 1.0)],
                    }),
                },
                TierSlot {
                    id: TierId(2),
                    relation: TierRelation::ChildOf { parent: TierId(1) },
                    tier: Tier::Point(PointTier {
                        name: "point-child".to_owned(),
                        points: vec![Point {
                            id: PointId(1),
                            time: 0.5,
                            label: String::new(),
                        }],
                    }),
                },
            ],
        );
        assert_single_issue(doc.validate(), |issue| {
            matches!(issue, IntegrityIssue::RelationTierKindMismatch { .. })
        });
    }

    #[test]
    fn validation_reports_aligned_boundary_missing() {
        let left = TierSlot {
            id: TierId(1),
            relation: TierRelation::AlignedBoundaries { with: TierId(2) },
            tier: Tier::Interval(IntervalTier {
                name: "left".to_owned(),
                intervals: vec![interval(1, 1, 2, 0.0, 0.5), interval(2, 2, 3, 0.5, 1.0)],
            }),
        };
        let right = TierSlot {
            id: TierId(2),
            relation: TierRelation::Independent,
            tier: Tier::Interval(IntervalTier {
                name: "right".to_owned(),
                intervals: vec![interval(3, 4, 5, 0.0, 1.0)],
            }),
        };
        let doc = Annotation::from_raw(0.0, 1.0, vec![left, right]);
        assert_single_issue(doc.validate(), |issue| {
            matches!(issue, IntegrityIssue::AlignedBoundaryMissing { .. })
        });
    }

    #[test]
    fn validation_reports_orphan_child_interval() {
        let parent = TierSlot {
            id: TierId(1),
            relation: TierRelation::Independent,
            tier: Tier::Interval(IntervalTier {
                name: "parent".to_owned(),
                intervals: vec![interval(1, 1, 2, 0.0, 0.5), interval(2, 2, 3, 0.5, 1.0)],
            }),
        };
        let child = TierSlot {
            id: TierId(2),
            relation: TierRelation::ChildOf { parent: TierId(1) },
            tier: Tier::Interval(IntervalTier {
                name: "child".to_owned(),
                intervals: vec![interval(3, 4, 5, 0.0, 1.0)],
            }),
        };
        let doc = Annotation::from_raw(0.0, 1.0, vec![parent, child]);
        assert_single_issue(doc.validate(), |issue| {
            matches!(issue, IntegrityIssue::OrphanChildInterval { .. })
        });
    }

    #[test]
    fn rejects_control_characters_in_labels() {
        let mut doc = basic_document();
        let target = first_interval_target(&doc, TierId(1));
        let err = doc.set_label(target, "bad\nlabel").unwrap_err();
        assert!(matches!(err, AnnotationError::InvalidLabelControl { .. }));
    }

    #[test]
    fn search_supports_ipa_combining_marks_and_regex() {
        let mut doc = Annotation::new(0.0, 1.0).unwrap();
        let tier = doc
            .add_interval_tier("ipa", TierRelation::Independent)
            .unwrap();
        let point_tier = doc
            .add_point_tier(
                "points",
                vec![(0.5, "tʃa\u{301}".to_owned())],
                TierRelation::Independent,
            )
            .unwrap();
        let interval_target = first_interval_target(&doc, tier);
        doc.set_label(interval_target, "a\u{301} ɬa").unwrap();

        let substring_hits = doc.search(&LabelQuery::substring("a\u{301}"));
        assert_eq!(substring_hits.len(), 2);
        assert_eq!(substring_hits[0].span, MatchSpan { start: 0, end: 3 });

        let regex_hits = doc.search(&LabelQuery::regex(r"\p{M}").in_tiers(vec![tier, point_tier]));
        assert_eq!(regex_hits.len(), 2);
        assert_eq!(regex_hits[0].span, MatchSpan { start: 1, end: 3 });
    }

    #[test]
    fn successful_random_mutation_sequences_keep_integrity() {
        run_random_sequences(false);
    }

    #[test]
    fn successful_random_mutation_sequences_restore_by_inverse() {
        run_random_sequences(true);
    }

    fn run_random_sequences(restore: bool) {
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };

        for _ in 0..300 {
            let mut doc = basic_document();
            let original = doc.clone();
            let mut inverses = Vec::new();
            let steps = 20 + (next() % 80) as usize;

            for _ in 0..steps {
                let operation = next() % 8;
                let inverse = match operation {
                    0 => random_insert(&mut doc, &mut next),
                    1 => random_move(&mut doc, &mut next),
                    2 => random_remove(&mut doc, &mut next),
                    3 => random_label(&mut doc, &mut next),
                    4 => random_insert_point(&mut doc, &mut next),
                    5 => random_move_point(&mut doc, &mut next),
                    6 => random_remove_point(&mut doc, &mut next),
                    _ => random_reorder(&mut doc, &mut next),
                };
                if let Some(inverse) = inverse {
                    assert_eq!(doc.validate(), Vec::new());
                    inverses.push(inverse);
                }
            }

            if restore {
                for inverse in inverses.iter().rev() {
                    doc.apply_inverse(inverse).unwrap();
                    assert_eq!(doc.validate(), Vec::new());
                }
                assert_eq!(doc, original);
            }
        }
    }

    fn basic_document() -> Annotation {
        let parent = TierSlot {
            id: TierId(1),
            relation: TierRelation::Independent,
            tier: Tier::Interval(IntervalTier {
                name: "parent".to_owned(),
                intervals: vec![interval(1, 1, 2, 0.0, 0.5), interval(2, 2, 3, 0.5, 1.0)],
            }),
        };
        let aligned = TierSlot {
            id: TierId(2),
            relation: TierRelation::AlignedBoundaries { with: TierId(1) },
            tier: Tier::Interval(IntervalTier {
                name: "aligned".to_owned(),
                intervals: vec![interval(3, 4, 5, 0.0, 0.5), interval(4, 5, 6, 0.5, 1.0)],
            }),
        };
        let child = TierSlot {
            id: TierId(3),
            relation: TierRelation::ChildOf { parent: TierId(1) },
            tier: Tier::Interval(IntervalTier {
                name: "child".to_owned(),
                intervals: vec![interval(5, 7, 8, 0.0, 0.5), interval(6, 8, 9, 0.5, 1.0)],
            }),
        };
        let points = TierSlot {
            id: TierId(4),
            relation: TierRelation::Independent,
            tier: Tier::Point(PointTier {
                name: "points".to_owned(),
                points: vec![
                    Point {
                        id: PointId(1),
                        time: 0.25,
                        label: "p".to_owned(),
                    },
                    Point {
                        id: PointId(2),
                        time: 0.75,
                        label: "q".to_owned(),
                    },
                ],
            }),
        };
        let doc = Annotation::from_raw(0.0, 1.0, vec![parent, aligned, child, points]);
        assert_eq!(doc.validate(), Vec::new());
        doc
    }

    fn random_insert<F>(doc: &mut Annotation, next: &mut F) -> Option<InverseMutation>
    where
        F: FnMut() -> u64,
    {
        let tier = match next() % 3 {
            0 => TierId(1),
            1 => TierId(2),
            _ => TierId(3),
        };
        let times = candidate_insert_times(doc, tier);
        if times.is_empty() {
            return None;
        }
        let at = times[(next() as usize) % times.len()];
        doc.insert_boundary(tier, at)
            .ok()
            .map(BoundaryId::inverse_insert)
    }

    fn random_move<F>(doc: &mut Annotation, next: &mut F) -> Option<InverseMutation>
    where
        F: FnMut() -> u64,
    {
        let boundaries = movable_boundaries(doc);
        if boundaries.is_empty() {
            return None;
        }
        let (boundary, left, right) = boundaries[(next() as usize) % boundaries.len()];
        let numerator = 1 + (next() % 8);
        let to = left + (right - left) * numerator as f64 / 9.0;
        doc.move_boundary(boundary, to, AlignMode::Linked)
            .ok()
            .map(|moved| moved.inverse())
    }

    fn random_remove<F>(doc: &mut Annotation, next: &mut F) -> Option<InverseMutation>
    where
        F: FnMut() -> u64,
    {
        let boundaries: Vec<BoundaryId> = movable_boundaries(doc)
            .into_iter()
            .map(|(boundary, _, _)| boundary)
            .collect();
        if boundaries.is_empty() {
            return None;
        }
        let boundary = boundaries[(next() as usize) % boundaries.len()];
        doc.remove_boundary(boundary)
            .ok()
            .map(|merged| merged.inverse())
    }

    fn random_label<F>(doc: &mut Annotation, next: &mut F) -> Option<InverseMutation>
    where
        F: FnMut() -> u64,
    {
        let targets = label_targets(doc);
        if targets.is_empty() {
            return None;
        }
        let target = targets[(next() as usize) % targets.len()];
        let text = format!("l{}", next() % 17);
        doc.set_label(target, &text)
            .ok()
            .map(|change| change.inverse())
    }

    fn random_insert_point<F>(doc: &mut Annotation, next: &mut F) -> Option<InverseMutation>
    where
        F: FnMut() -> u64,
    {
        let tiers = point_tiers(doc);
        if tiers.is_empty() {
            return None;
        }
        let (tier, points) = tiers[(next() as usize) % tiers.len()].clone();
        let times = point_insert_times(doc, &points);
        if times.is_empty() {
            return None;
        }
        let at = times[(next() as usize) % times.len()];
        let label = format!("p{}", next() % 17);
        doc.insert_point(tier, at, &label)
            .ok()
            .map(|insertion| insertion.inverse())
    }

    fn random_move_point<F>(doc: &mut Annotation, next: &mut F) -> Option<InverseMutation>
    where
        F: FnMut() -> u64,
    {
        let movable = movable_points(doc);
        if movable.is_empty() {
            return None;
        }
        let (point, lower, upper) = movable[(next() as usize) % movable.len()];
        let numerator = 1 + (next() % 8);
        let to = lower + (upper - lower) * numerator as f64 / 9.0;
        if to.to_bits() == lower.to_bits() || to.to_bits() == upper.to_bits() {
            return None;
        }
        doc.move_point(point, to).ok().map(|moved| moved.inverse())
    }

    fn random_remove_point<F>(doc: &mut Annotation, next: &mut F) -> Option<InverseMutation>
    where
        F: FnMut() -> u64,
    {
        let ids: Vec<PointId> = point_tiers(doc)
            .into_iter()
            .flat_map(|(_tier, points)| points.into_iter().map(|point| point.id))
            .collect();
        if ids.is_empty() {
            return None;
        }
        let id = ids[(next() as usize) % ids.len()];
        doc.remove_point(id).ok().map(|removal| removal.inverse())
    }

    fn random_reorder<F>(doc: &mut Annotation, next: &mut F) -> Option<InverseMutation>
    where
        F: FnMut() -> u64,
    {
        let count = doc.tiers().len();
        if count < 2 {
            return None;
        }
        let tier = doc.tiers()[(next() as usize) % count].id;
        let to_index = (next() as usize) % count;
        doc.reorder_tier(tier, to_index)
            .ok()
            .map(|reorder| reorder.inverse())
    }

    fn point_tiers(doc: &Annotation) -> Vec<(TierId, Vec<Point>)> {
        doc.tiers()
            .iter()
            .filter_map(|slot| match &slot.tier {
                Tier::Point(tier) => Some((slot.id, tier.points.clone())),
                Tier::Interval(_) => None,
            })
            .collect()
    }

    fn point_insert_times(doc: &Annotation, points: &[Point]) -> Vec<f64> {
        let mut fence = vec![doc.xmin()];
        fence.extend(points.iter().map(|point| point.time));
        fence.push(doc.xmax());
        fence
            .windows(2)
            .filter_map(|pair| {
                let mid = (pair[0] + pair[1]) / 2.0;
                (mid.to_bits() != pair[0].to_bits() && mid.to_bits() != pair[1].to_bits())
                    .then_some(mid)
            })
            .collect()
    }

    fn movable_points(doc: &Annotation) -> Vec<(PointId, f64, f64)> {
        let mut out = Vec::new();
        for (_tier, points) in point_tiers(doc) {
            for (index, point) in points.iter().enumerate() {
                let lower = if index > 0 {
                    points[index - 1].time
                } else {
                    doc.xmin()
                };
                let upper = if index + 1 < points.len() {
                    points[index + 1].time
                } else {
                    doc.xmax()
                };
                if upper > lower {
                    out.push((point.id, lower, upper));
                }
            }
        }
        out
    }

    fn candidate_insert_times(doc: &Annotation, tier: TierId) -> Vec<f64> {
        let Some(slot) = doc.tier(tier) else {
            return Vec::new();
        };
        let Tier::Interval(tier) = &slot.tier else {
            return Vec::new();
        };
        tier.intervals
            .iter()
            .filter_map(|interval| {
                let midpoint = (interval.xmin + interval.xmax) / 2.0;
                (midpoint > interval.xmin && midpoint < interval.xmax).then_some(midpoint)
            })
            .collect()
    }

    fn movable_boundaries(doc: &Annotation) -> Vec<(BoundaryId, f64, f64)> {
        doc.tiers()
            .iter()
            .filter_map(|slot| match &slot.tier {
                Tier::Interval(tier) => Some(tier),
                Tier::Point(_) => None,
            })
            .flat_map(|tier| {
                tier.intervals
                    .windows(2)
                    .map(|pair| (pair[0].end_boundary, pair[0].xmin, pair[1].xmax))
            })
            .collect()
    }

    fn label_targets(doc: &Annotation) -> Vec<LabelTarget> {
        let mut targets = Vec::new();
        for slot in doc.tiers() {
            match &slot.tier {
                Tier::Interval(tier) => {
                    targets.extend(tier.intervals.iter().map(|interval| LabelTarget::Interval {
                        tier: slot.id,
                        interval: interval.id,
                    }));
                }
                Tier::Point(tier) => {
                    targets.extend(tier.points.iter().map(|point| LabelTarget::Point {
                        tier: slot.id,
                        point: point.id,
                    }));
                }
            }
        }
        targets
    }

    fn first_interval_target(doc: &Annotation, tier: TierId) -> LabelTarget {
        let slot = doc.tier(tier).unwrap();
        let Tier::Interval(tier_data) = &slot.tier else {
            panic!("expected interval tier");
        };
        LabelTarget::Interval {
            tier,
            interval: tier_data.intervals[0].id,
        }
    }

    fn raw_interval_doc(intervals: Vec<(u64, u64, u64, f64, f64)>) -> Annotation {
        Annotation::from_raw(
            0.0,
            1.0,
            vec![TierSlot {
                id: TierId(1),
                relation: TierRelation::Independent,
                tier: Tier::Interval(IntervalTier {
                    name: "raw".to_owned(),
                    intervals: intervals
                        .into_iter()
                        .map(|(id, start, end, xmin, xmax)| interval(id, start, end, xmin, xmax))
                        .collect(),
                }),
            }],
        )
    }

    fn interval(id: u64, start: u64, end: u64, xmin: f64, xmax: f64) -> Interval {
        Interval {
            id: IntervalId(id),
            start_boundary: BoundaryId(start),
            end_boundary: BoundaryId(end),
            xmin,
            xmax,
            label: String::new(),
        }
    }

    fn empty_point_slot(id: u64, name: &str) -> TierSlot {
        TierSlot {
            id: TierId(id),
            relation: TierRelation::Independent,
            tier: Tier::Point(PointTier {
                name: name.to_owned(),
                points: Vec::new(),
            }),
        }
    }

    fn assert_single_issue<F>(issues: Vec<IntegrityIssue>, predicate: F)
    where
        F: FnOnce(&IntegrityIssue) -> bool,
    {
        assert_eq!(issues.len(), 1, "unexpected issues: {issues:?}");
        assert!(predicate(&issues[0]), "unexpected issue: {:?}", issues[0]);
    }
}
