use super::error::DatabaseError;

const MAX_USERNAME_SIZE: usize = 15;
const MIN_USERNAME_SIZE: usize = 4;

const MAX_NAME_SIZE: usize = 60;
const MIN_NAME_SIZE: usize = 1;

pub fn validate_username(username: &str) -> Result<(), DatabaseError> {
    if !username.starts_with('@') {
        return Err(DatabaseError::from("Username must start with '@' symbol!"))
    }
    
    if username.len() > MAX_USERNAME_SIZE || username.len() < MIN_USERNAME_SIZE {
        return Err(DatabaseError::from("Invalid username length"))
    }

    Ok(())
}

pub fn validate_name(name: &str) -> Result<(), DatabaseError> {
    if name.len() > MAX_NAME_SIZE || name.len() < MIN_NAME_SIZE {
        return Err(DatabaseError::from("Invalid name length"))
    }

    Ok(())
}