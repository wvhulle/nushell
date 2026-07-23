use lsp_types::{Position, Range};
use std::ops::Range as ByteRange;

pub(crate) fn byte_offset_at_position(text: &str, position: Position) -> usize {
    let line_start: usize = text
        .split_inclusive('\n')
        .take(position.line as usize)
        .map(str::len)
        .sum();
    let line = text
        .get(line_start..)
        .and_then(|remaining| remaining.split('\n').next())
        .unwrap_or_default();
    let target_units = position.character as usize;
    let offset_in_line = line
        .char_indices()
        .scan(0usize, |consumed_units, (byte_index, character)| {
            let units_before = *consumed_units;
            *consumed_units += character.len_utf16();
            Some((byte_index, units_before))
        })
        .find(|(_, units_before)| *units_before >= target_units)
        .map_or(line.len(), |(byte_index, _)| byte_index);
    line_start + offset_in_line
}

pub(crate) fn byte_range_of_lsp_range(text: &str, range: &Range) -> ByteRange<usize> {
    let start = byte_offset_at_position(text, range.start);
    let end = byte_offset_at_position(text, range.end);
    start..end.max(start)
}

fn position_at_byte_offset(text: &str, offset: usize) -> Position {
    let (line, character) = text
        .char_indices()
        .take_while(|(byte_index, _)| *byte_index < offset)
        .fold((0u32, 0u32), |(line, character), (_, current)| {
            if current == '\n' {
                (line + 1, 0)
            } else {
                (
                    line,
                    character + u32::try_from(current.len_utf16()).unwrap_or(1),
                )
            }
        });
    Position { line, character }
}

pub(crate) fn lsp_range_of_byte_range(text: &str, byte_range: &ByteRange<usize>) -> Range {
    Range {
        start: position_at_byte_offset(text, byte_range.start),
        end: position_at_byte_offset(text, byte_range.end),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn position(line: u32, character: u32) -> Position {
        Position { line, character }
    }

    #[test]
    fn ascii_offsets_match_columns() {
        assert_eq!(byte_offset_at_position("let x", position(0, 4)), 4);
    }

    #[test]
    fn multibyte_characters_are_measured_in_utf16_units() {
        assert_eq!(byte_offset_at_position("é x", position(0, 2)), 3);
    }

    #[test]
    fn surrogate_pairs_count_as_two_units() {
        assert_eq!(byte_offset_at_position("😀x", position(0, 2)), 4);
    }

    #[test]
    fn position_inside_surrogate_pair_lands_on_a_character_boundary() {
        let text = "😀x";
        let offset = byte_offset_at_position(text, position(0, 1));
        assert!(text.is_char_boundary(offset));
    }

    #[test]
    fn later_lines_start_after_the_newline() {
        assert_eq!(byte_offset_at_position("é\nab", position(1, 1)), 4);
    }

    #[test]
    fn positions_past_the_end_clamp_to_the_text() {
        assert_eq!(byte_offset_at_position("ab", position(0, 99)), 2);
        assert_eq!(byte_offset_at_position("ab", position(7, 0)), 2);
    }

    #[test]
    fn inverted_ranges_become_empty() {
        let inverted = Range {
            start: position(0, 2),
            end: position(0, 1),
        };
        assert_eq!(byte_range_of_lsp_range("abc", &inverted), 2..2);
    }

    #[test]
    fn byte_ranges_round_trip_through_lsp_ranges() {
        let text = "é😀\nabc";
        let byte_range = 2..8;
        let lsp_range = lsp_range_of_byte_range(text, &byte_range);
        assert_eq!(lsp_range.start, position(0, 1));
        assert_eq!(lsp_range.end, position(1, 1));
        assert_eq!(byte_range_of_lsp_range(text, &lsp_range), byte_range);
    }
}
