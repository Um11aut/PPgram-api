use std::ops::{Range, RangeBounds};
use std::mem::swap;

use crate::db::chat::messages::MESSAGES_DB;

use super::error::PPError;

const MAX_USERNAME_SIZE: usize = 30;
const MIN_USERNAME_SIZE: usize = 4;

const MAX_NAME_SIZE: usize = 60;
const MIN_NAME_SIZE: usize = 1;

pub fn validate_username(username: &str) -> Result<(), PPError> {
    if !username.starts_with('@') {
        return Err(PPError::from("Username must start with '@' symbol!"))
    }
    
    if username.len() > MAX_USERNAME_SIZE || username.len() < MIN_USERNAME_SIZE {
        return Err(PPError::from("Invalid username length"))
    }

    Ok(())
}

pub fn validate_name(name: &str) -> Result<(), PPError> {
    if name.len() > MAX_NAME_SIZE || name.len() < MIN_NAME_SIZE {
        return Err(PPError::from("Invalid name length"))
    }

    Ok(())
}

/// Makes range valid. 
pub fn validate_range(range: impl RangeBounds<i32>) -> Result<(i32, i32), PPError> {
    match (range.start_bound(), range.end_bound()) {
        (std::ops::Bound::Included(&start), std::ops::Bound::Included(&end)) => {
            let (mut start, mut end) = (start.clone(), end.clone());

            if end.is_negative() {
                let real_start = start + end;
                let real_end = start;
                start = real_start;
                end = real_end;
            }

            if start.is_negative() {
                start = 0;
            }

            return Ok((start, end));
        }
        _ => Err(PPError::from("Range must be exclusive!")),
    }
}