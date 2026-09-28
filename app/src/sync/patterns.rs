//! 상대 경로 기준 제외 glob 비교. `*`와 `?`는 조각 내부, `**`는 폴더 깊이를 넘는다.

use std::collections::HashMap;

pub(super) fn matches_exclude_pattern(pattern: &str, relative_path: &str) -> bool {
    let pattern = pattern.trim();
    if pattern.is_empty() {
        return false;
    }

    let pattern_segments: Vec<_> = pattern
        .split(['/', '\\'])
        .filter(|segment| !segment.is_empty() && *segment != ".")
        .collect();
    let path_segments: Vec<_> = relative_path
        .split(['/', '\\'])
        .filter(|segment| !segment.is_empty() && *segment != ".")
        .collect();

    let mut memo = HashMap::new();
    match_glob_segments(&pattern_segments, &path_segments, 0, 0, &mut memo)
}

fn match_glob_segments(
    pattern: &[&str],
    path: &[&str],
    pattern_index: usize,
    path_index: usize,
    memo: &mut HashMap<(usize, usize), bool>,
) -> bool {
    if let Some(result) = memo.get(&(pattern_index, path_index)) {
        return *result;
    }

    let result = if pattern_index == pattern.len() {
        path_index == path.len()
    } else if is_globstar(pattern[pattern_index]) {
        match_glob_segments(pattern, path, pattern_index + 1, path_index, memo)
            || (path_index < path.len()
                && match_glob_segments(pattern, path, pattern_index, path_index + 1, memo))
    } else {
        path_index < path.len()
            && match_glob_segment(pattern[pattern_index], path[path_index])
            && match_glob_segments(pattern, path, pattern_index + 1, path_index + 1, memo)
    };

    memo.insert((pattern_index, path_index), result);
    result
}

fn is_globstar(segment: &str) -> bool {
    segment.len() >= 2 && segment.chars().all(|character| character == '*')
}

fn match_glob_segment(pattern: &str, text: &str) -> bool {
    let pattern: Vec<_> = pattern.chars().collect();
    let text: Vec<_> = text.chars().collect();
    let mut pattern_index = 0;
    let mut text_index = 0;
    let mut last_star = None;
    let mut star_text_index = 0;

    while text_index < text.len() {
        if pattern_index < pattern.len()
            && (pattern[pattern_index] == '?' || pattern[pattern_index] == text[text_index])
        {
            pattern_index += 1;
            text_index += 1;
        } else if pattern_index < pattern.len() && pattern[pattern_index] == '*' {
            last_star = Some(pattern_index);
            star_text_index = text_index;
            pattern_index += 1;
        } else if let Some(star_index) = last_star {
            pattern_index = star_index + 1;
            star_text_index += 1;
            text_index = star_text_index;
        } else {
            return false;
        }
    }

    while pattern_index < pattern.len() && pattern[pattern_index] == '*' {
        pattern_index += 1;
    }
    pattern_index == pattern.len()
}
