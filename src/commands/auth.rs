use argon2::password_hash::{rand_core::OsRng, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Argon2, PasswordHash};

pub fn make_hash(passwd: &str) -> Result<String, std::io::Error> {
    let salt = SaltString::generate(&mut OsRng);

    let argon2 = Argon2::default(); // argon2id v19

    Ok(argon2
        .hash_password(passwd.as_bytes(), &salt)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?
        .to_string())
}

pub fn check_password(passwd: &str, hash: &str) -> Result<bool, std::io::Error> {
    let parsed_hash = PasswordHash::new(hash)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    Ok(Argon2::default()
        .verify_password(passwd.as_bytes(), &parsed_hash)
        .is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_passwords_check() {
        let passwd = "password1234";
        let the_hash = make_hash(passwd);
        assert!(the_hash.is_ok());
        let checked = check_password(passwd, &the_hash.unwrap());
        assert!(checked.is_ok());
        assert!(checked.unwrap());
    }
    #[test]
    fn unequal_passwords_fail() {
        let the_hash = make_hash("password1234");
        assert!(the_hash.is_ok());
        let checked = check_password("peepeepoopoo", &the_hash.unwrap());
        assert!(checked.is_ok());
        assert!(!checked.unwrap());
    }
}
