use crate::project::{AudioItem, CaptionItem, Time, VideoItem};

pub trait TimeSlice {
    fn start(&self) -> Time;
    fn end(&self) -> Time;

    fn duration_nanos(&self) -> u64 {
        self.end()
            .as_nonnegative_nanos()
            .saturating_sub(self.start().as_nonnegative_nanos())
    }
}

impl TimeSlice for CaptionItem {
    fn start(&self) -> Time {
        self.start
    }

    fn end(&self) -> Time {
        self.end
    }
}

impl TimeSlice for VideoItem {
    fn start(&self) -> Time {
        self.start
    }

    fn end(&self) -> Time {
        self.end
    }
}

impl TimeSlice for AudioItem {
    fn start(&self) -> Time {
        self.start
    }

    fn end(&self) -> Time {
        self.end
    }
}

pub fn overlapping<'a, T: TimeSlice>(
    items: &'a [T],
    start: Time,
    end: Time,
) -> impl Iterator<Item = (usize, &'a T)> + 'a {
    let range_start = start.min(end);
    let range_end = start.max(end);
    let last = items.partition_point(|item| item.start() <= range_end);

    items[..last]
        .iter()
        .enumerate()
        .filter(move |(_, item)| item.end() >= range_start)
}

pub fn collides<T: TimeSlice>(items: &[T], start: Time, end: Time) -> bool {
    let range_start = start.min(end);
    let range_end = start.max(end);
    overlapping(items, range_start, range_end)
        .any(|(_, item)| item.start() < range_end && item.end() > range_start)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy)]
    struct Slice {
        start: Time,
        end: Time,
    }

    impl Slice {
        fn new(start: i64, end: i64) -> Self {
            Self {
                start: Time::from_seconds(start),
                end: Time::from_seconds(end),
            }
        }
    }

    impl TimeSlice for Slice {
        fn start(&self) -> Time {
            self.start
        }

        fn end(&self) -> Time {
            self.end
        }
    }

    #[test]
    fn finds_overlapping_items_in_sorted_track() {
        let items = [Slice::new(0, 2), Slice::new(3, 5), Slice::new(6, 8)];
        let indices: Vec<_> = overlapping(&items, Time::from_seconds(4), Time::from_seconds(7))
            .map(|(index, _)| index)
            .collect();

        assert_eq!(indices, [1, 2]);
    }

    #[test]
    fn accepts_reversed_bounds() {
        let items = [Slice::new(0, 2), Slice::new(3, 5), Slice::new(6, 8)];
        let indices: Vec<_> = overlapping(&items, Time::from_seconds(7), Time::from_seconds(4))
            .map(|(index, _)| index)
            .collect();

        assert_eq!(indices, [1, 2]);
    }

    #[test]
    fn keeps_item_that_starts_before_range_and_extends_into_it() {
        let items = [Slice::new(0, 10), Slice::new(2, 3), Slice::new(12, 14)];
        let indices: Vec<_> = overlapping(&items, Time::from_seconds(8), Time::from_seconds(9))
            .map(|(index, _)| index)
            .collect();

        assert_eq!(indices, [0]);
    }

    #[test]
    fn ignores_items_starting_after_range() {
        let items = [Slice::new(0, 1), Slice::new(5, 6), Slice::new(9, 10)];
        let indices: Vec<_> = overlapping(&items, Time::from_seconds(2), Time::from_seconds(4))
            .map(|(index, _)| index)
            .collect();

        assert!(indices.is_empty());
    }

    #[test]
    fn detects_real_collision() {
        let items = [Slice::new(0, 2), Slice::new(4, 6)];

        assert!(collides(
            &items,
            Time::from_seconds(1),
            Time::from_seconds(3)
        ));
    }

    #[test]
    fn allows_touching_edges() {
        let items = [Slice::new(0, 2), Slice::new(4, 6)];

        assert!(!collides(
            &items,
            Time::from_seconds(2),
            Time::from_seconds(4)
        ));
    }
}
