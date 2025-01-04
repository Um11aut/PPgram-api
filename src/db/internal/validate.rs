use std::ops::RangeBounds;


use super::error::PPError;

const MAX_USERNAME_SIZE: usize = 30;
const MIN_USERNAME_SIZE: usize = 3;

const MAX_NAME_SIZE: usize = 60;
const MIN_NAME_SIZE: usize = 1;

pub fn validate_username(username: &str) -> Result<(), PPError> {
    let lowercase: Vec<char> = ('a'..='z').collect();
    let uppercase: Vec<char> = ('A'..='Z').collect();

    let mut allowed_characters: Vec<char> = lowercase.iter().chain(uppercase.iter()).copied().collect();
    allowed_characters.push('_');
    let allowed_set: std::collections::HashSet<_> = allowed_characters.iter().collect();

    let username_body = &username[1..]; // Exclude '@' for character validation

    if !username_body.chars().all(|c| allowed_set.contains(&c)) {
        return Err(PPError::from("Invalid Username characters provided! Allowed are: a..z, A..Z, _"));
    }

    if !username.starts_with('@') {
        return Err(PPError::from("Username must start with '@' symbol!"))
    }

    if username.len() > MAX_USERNAME_SIZE {
        return Err(PPError::from("Username too big"))
    }

    if username.len() < MIN_USERNAME_SIZE {
        return Err(PPError::from("Username too small"))
    }

    Ok(())
}

pub fn validate_name(name: &str) -> Result<(), PPError> {
    if name.len() > MAX_NAME_SIZE {
        return Err(PPError::from("Name too big"))
    }

    if name.len() < MIN_NAME_SIZE {
        return Err(PPError::from("Name too small"))
    }

    Ok(())
}

/// Makes range valid.
pub fn validate_range(range: impl RangeBounds<i32>) -> Result<(i32, i32), PPError> {
    match (range.start_bound(), range.end_bound()) {
        (std::ops::Bound::Included(&start), std::ops::Bound::Included(&end)) => {
            let (mut start, mut end) = (start, end);

            if end.is_negative() {
                let real_start = start + end;
                let real_end = start;
                start = real_start;
                end = real_end;
            }

            if start.is_negative() {
                start = 0;
            }

            Ok((start, end))
        }
        _ => Err(PPError::from("Range must be exclusive!")),
    }
}
