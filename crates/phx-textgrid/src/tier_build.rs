//! Shared tier-assembly helpers used by both the text and binary readers.
//!
//! Neither source format carries an explicit boundary identifier: adjacent
//! intervals within a tier simply repeat the same time value for one
//! interval's `xmax` and the next interval's `xmin`. Minting a shared
//! [`BoundaryId`] at each such join, and fresh document-wide identifiers for
//! every tier, interval, and point, is the one piece of bookkeeping both
//! readers need in exactly the same shape, so it lives here once.

use phx_annot::{BoundaryId, Interval, IntervalId, IntervalTier, Point, PointId, PointTier};

/// Mints document-wide unique identifiers so that boundary, interval, and point
/// identifiers never collide across tiers, which `Annotation::validate` checks.
#[derive(Default)]
pub(crate) struct IdMinter {
    tier: u64,
    boundary: u64,
    interval: u64,
    point: u64,
}

impl IdMinter {
    pub(crate) fn next_tier(&mut self) -> u64 {
        self.tier += 1;
        self.tier
    }

    pub(crate) fn next_boundary(&mut self) -> u64 {
        self.boundary += 1;
        self.boundary
    }

    pub(crate) fn next_interval(&mut self) -> u64 {
        self.interval += 1;
        self.interval
    }

    pub(crate) fn next_point(&mut self) -> u64 {
        self.point += 1;
        self.point
    }
}

/// Assembles an interval tier from raw `(xmin, xmax, label)` triples read in
/// document order, minting a boundary identifier shared between each
/// interval's end and the next interval's start.
pub(crate) fn build_interval_tier(
    name: String,
    tier_xmin: f64,
    tier_xmax: f64,
    raw: Vec<(f64, f64, String)>,
    ids: &mut IdMinter,
) -> IntervalTier {
    let mut intervals = Vec::with_capacity(raw.len());
    let mut shared_boundary: Option<u64> = None;
    for (xmin, xmax, label) in raw {
        let start = shared_boundary.unwrap_or_else(|| ids.next_boundary());
        let end = ids.next_boundary();
        shared_boundary = Some(end);
        intervals.push(Interval {
            id: IntervalId::new(ids.next_interval()),
            start_boundary: BoundaryId::new(start),
            end_boundary: BoundaryId::new(end),
            xmin,
            xmax,
            label,
        });
    }
    IntervalTier {
        name,
        xmin: tier_xmin,
        xmax: tier_xmax,
        intervals,
    }
}

/// Assembles a point tier from raw `(time, label)` pairs read in document order.
pub(crate) fn build_point_tier(
    name: String,
    tier_xmin: f64,
    tier_xmax: f64,
    raw: Vec<(f64, String)>,
    ids: &mut IdMinter,
) -> PointTier {
    let points = raw
        .into_iter()
        .map(|(time, label)| Point {
            id: PointId::new(ids.next_point()),
            time,
            label,
        })
        .collect();
    PointTier {
        name,
        xmin: tier_xmin,
        xmax: tier_xmax,
        points,
    }
}
