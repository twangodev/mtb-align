//! Checks each layer against answers recorded from OpenCV's `AlignMTB`, whose
//! conventions this follows, so a disagreement is a bug here, not a difference
//! of opinion. Regenerate with `tests/fixtures/generate.py`.
//!
//! The layers are separate on purpose: `calculateShift` failing alone means the
//! search tipped a near-tie, not that a convention is wrong.

use mtb_align::{Gray, Options, Percentile, Shift, Shrink, compute_bitmaps, shift, shrink2};

const FIXTURES: &str = include_str!("fixtures/opencv.txt");

enum Case {
    Bitmaps {
        gray: Gray,
        tolerance: u8,
        threshold: Vec<bool>,
        exclusion: Vec<bool>,
    },
    Shift {
        gray: Gray,
        by: Shift,
        expected: Gray,
    },
    Shrink {
        gray: Gray,
        expected: Gray,
    },
    Align {
        reference: Gray,
        target: Gray,
        options: Options,
        expected: Shift,
    },
}

impl Case {
    fn label(&self) -> String {
        match self {
            Case::Bitmaps { gray, .. } => format!("bitmaps {}x{}", gray.width(), gray.height()),
            Case::Shift { gray, by, .. } => {
                format!(
                    "shift {}x{} by ({}, {})",
                    gray.width(),
                    gray.height(),
                    by.x,
                    by.y
                )
            }
            Case::Shrink { gray, .. } => format!("shrink {}x{}", gray.width(), gray.height()),
            Case::Align {
                reference, options, ..
            } => format!(
                "align {}x{} with {} bits",
                reference.width(),
                reference.height(),
                options.bits
            ),
        }
    }
}

fn parse() -> Vec<Case> {
    let mut lines = FIXTURES
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty());
    let mut cases = Vec::new();

    while let Some(header) = lines.next() {
        let fields: Vec<&str> = header.split_whitespace().collect();
        assert_eq!(fields[0], "CASE", "unexpected line: {header}");

        let number = |field: &str| field.parse::<usize>().expect("dimension");
        let signed = |field: &str| field.parse::<i32>().expect("offset");
        let (width, height) = (number(fields[2]), number(fields[3]));

        let mut tagged = |tag: &str| {
            let line = lines.next().unwrap_or_else(|| panic!("missing {tag} line"));
            let (found, payload) = line.split_once(' ').expect("tagged line");
            assert_eq!(found, tag, "expected {tag}, found {found}");
            payload
        };

        cases.push(match fields[1] {
            "bitmaps" => Case::Bitmaps {
                tolerance: number(fields[4]) as u8,
                gray: Gray::from_vec(unhex(tagged("IN")), width, height),
                threshold: unbits(tagged("TB")),
                exclusion: unbits(tagged("EB")),
            },
            "shift" => Case::Shift {
                by: Shift::new(signed(fields[4]), signed(fields[5])),
                gray: Gray::from_vec(unhex(tagged("IN")), width, height),
                expected: Gray::from_vec(unhex(tagged("OUT")), width, height),
            },
            "shrink" => Case::Shrink {
                gray: Gray::from_vec(unhex(tagged("IN")), width, height),
                expected: Gray::from_vec(
                    unhex(tagged("OUT")),
                    number(fields[4]),
                    number(fields[5]),
                ),
            },
            "align" => Case::Align {
                options: Options {
                    bits: number(fields[4]) as u32,
                    tolerance: number(fields[5]) as u8,
                    percentile: Percentile::MEDIAN,
                    ..Options::opencv()
                },
                reference: Gray::from_vec(unhex(tagged("REF")), width, height),
                target: Gray::from_vec(unhex(tagged("TGT")), width, height),
                expected: {
                    let fields: Vec<&str> = tagged("OUT").split_whitespace().collect();
                    Shift::new(signed(fields[0]), signed(fields[1]))
                },
            },
            other => panic!("unknown case {other}"),
        });
    }

    cases
}

fn unhex(payload: &str) -> Vec<u8> {
    payload
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            u8::from_str_radix(std::str::from_utf8(pair).expect("ascii"), 16).expect("hex byte")
        })
        .collect()
}

fn unbits(payload: &str) -> Vec<bool> {
    payload.chars().map(|bit| bit == '1').collect()
}

/// Reports the first disagreement: a wrong convention breaks thousands.
fn compare(actual: impl Fn(usize, usize) -> bool, expected: &[bool], width: usize, what: &str) {
    for (index, &want) in expected.iter().enumerate() {
        let (x, y) = (index % width, index / width);
        assert_eq!(
            actual(x, y),
            want,
            "{what} disagrees at ({x}, {y}): mtb-align {}, OpenCV {want}",
            actual(x, y)
        );
    }
}

#[test]
fn the_fixtures_cover_every_layer() {
    let cases = parse();
    let count = |kind: fn(&Case) -> bool| cases.iter().filter(|case| kind(case)).count();

    assert!(count(|case| matches!(case, Case::Bitmaps { .. })) >= 4);
    assert!(count(|case| matches!(case, Case::Shift { .. })) >= 4);
    assert!(count(|case| matches!(case, Case::Shrink { .. })) >= 4);
    assert!(count(|case| matches!(case, Case::Align { .. })) >= 2);
}

#[test]
fn the_bitmaps_agree_with_opencv() {
    for case in &parse() {
        let Case::Bitmaps {
            gray,
            tolerance,
            threshold,
            exclusion,
        } = case
        else {
            continue;
        };

        let bitmaps = compute_bitmaps(gray, Percentile::MEDIAN, *tolerance);
        let width = gray.width();

        compare(
            |x, y| bitmaps.threshold.get(x, y),
            threshold,
            width,
            &format!("{}: the threshold bitmap", case.label()),
        );
        compare(
            |x, y| bitmaps.exclusion.get(x, y),
            exclusion,
            width,
            &format!("{}: the exclusion bitmap", case.label()),
        );
    }
}

#[test]
fn shifting_agrees_with_opencv() {
    for case in &parse() {
        let Case::Shift { gray, by, expected } = case else {
            continue;
        };

        assert_eq!(
            gray.shifted(*by).as_slice(),
            expected.as_slice(),
            "{}",
            case.label()
        );
    }
}

/// Pins the subsample convention, which the search cannot: it converges on the
/// same offset whichever corner of each block is taken.
#[test]
fn shrinking_agrees_with_opencv() {
    for case in &parse() {
        let Case::Shrink { gray, expected } = case else {
            continue;
        };

        let shrunk = shrink2(gray, Shrink::Subsample);

        assert_eq!(
            (shrunk.width(), shrunk.height()),
            (expected.width(), expected.height()),
            "{}: wrong size",
            case.label()
        );
        assert_eq!(shrunk.as_slice(), expected.as_slice(), "{}", case.label());
    }
}

#[test]
fn the_search_agrees_with_opencv() {
    for case in &parse() {
        let Case::Align {
            reference,
            target,
            options,
            expected,
        } = case
        else {
            continue;
        };

        assert_eq!(
            shift(reference, target, options).unwrap(),
            *expected,
            "{}",
            case.label()
        );
    }
}
