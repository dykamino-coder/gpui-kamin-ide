//! Тесты: дифференциальное покрытие арифметики разрешённых SVG-команд.
use super::{Command::*, Geometry, Sink};

#[test]
fn direct_commands_match_svg_parser_points() {
    for relative in [false, true] {
        for fraction in [0.0, 0.1, 0.33333334, 0.00001234567] {
            let commands = [
                Move([2.3 + fraction, 3.7]),
                Line([12.6, 5.2]),
                Horizontal(20.1),
                Vertical(14.9),
                Cubic([21.4, 18.3, 12.2, 25.7, 7.9, 20.4]),
                SmoothCubic([3.8, 14.2, 6.1, 9.7]),
                Quadratic([8.2, 4.3, 11.4, 8.8]),
                SmoothQuadratic([16.7, 12.9]),
                Arc([7.3, 4.9, 37.2, 1.0, 0.0, 23.8, 19.1]),
                Arc([0.0, 3.0, 0.0, 0.0, 1.0, 12.0, 10.0]),
                Close,
                Line([14.3, 11.9]),
                Close,
            ];
            let (mut text, mut path) = (String::new(), Geometry::default());
            for c in commands {
                text.command(relative, c);
                path.command(relative, c);
            }
            assert_eq!(path.finish(), Geometry::parse(&text), "{text}");
        }
    }
}

#[test]
fn arcs_cover_all_flag_combinations_and_fractional_radii() {
    for large in [0.0, 1.0] {
        for sweep in [0.0, 1.0] {
            for rx in [0.1, 7.123456, 90.0] {
                let (mut text, mut path) = (String::new(), Geometry::default());
                for c in [
                    Move([3.1, 4.7]),
                    Arc([rx, 9.321, 23.5, large, sweep, 40.3, 31.9]),
                    Close,
                ] {
                    text.command(false, c);
                    path.command(false, c);
                }
                assert_eq!(path.finish(), Geometry::parse(&text), "{text}");
            }
        }
    }
}
