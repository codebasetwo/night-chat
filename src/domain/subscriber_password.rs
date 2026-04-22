use argon2::{
    password_hash::{
        rand_core::OsRng,
        PasswordHash,
        PasswordHasher,
        PasswordVerifier,
        SaltString,
    },
    Argon2,
};

#[derive(Debug)]
pub struct SubscriberPassword {
    pub hashed_password: Vec<u8>,
    pub plaintext_password: String,
}

impl SubscriberPassword {
    pub fn parse(pw: String) -> Result<SubscriberPassword, String> {
        // Password is no empty string
        if pw.is_empty() {
            return Err("Password cannot be empty".to_string())
        }
        // password is greater or equal to 8
        if pw.len() < 8 && pw.len() > 32  {
            return Err("Password must be at most 32 characters and at least 8 characters long".to_string())
        } 

        // Password has atleast one uppercase letter
        let has_uppercase = pw.chars().any(|c| c.is_uppercase());
        // Password has atleast one special character
        let has_special_character = pw.chars().any(|c| !c.is_alphanumeric());

        if !has_uppercase || !has_special_character {
            return Err(
                "Password must contain atleast one special character and uppercase character".to_string()
            )
        }

        let subscriber_password = SubscriberPassword::new(&pw);
        Ok(subscriber_password)
    } 
}

impl SubscriberPassword {
    pub fn new(plaintext_password: &str) -> Self {
        let hashed_password = SubscriberPassword::hash_password(&plaintext_password.as_bytes())
            .unwrap();
        let plaintext_password = plaintext_password.to_string();
        Self { plaintext_password, hashed_password }
    }

    fn hash_password(plaintext_password: &[u8]) -> Result<Vec<u8>, argon2::password_hash::Error> {
        // Use argon2 to hash the password
        let salt = SaltString::generate(&mut OsRng);
        let argon = Argon2::default();
        let password_hash = argon.hash_password(plaintext_password, &salt)?.to_string().into_bytes();
        Ok(password_hash)
    }

    pub fn verify_password(
        plaintext_password: &[u8], 
        hashed_password: String) -> Result<bool, argon2::password_hash::Error> {
        let parsed_hash = PasswordHash::new(&hashed_password)?;
        Ok(Argon2::default().verify_password(plaintext_password, &parsed_hash).is_ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_password() {
        assert!(SubscriberPassword::parse("Pass123!".to_string()).is_ok());
        assert!(SubscriberPassword::parse("MyP@ssw0rd".to_string()).is_ok());
    }

    #[test]
    fn empty_password() {
        assert!(SubscriberPassword::parse("".to_string()).is_err());
    }

    #[test]
    fn too_short() {
        assert!(SubscriberPassword::parse("Pass1!".to_string()).is_err());
    }

    #[test]
    fn too_long() {
        let long_pass = "P@ssw0rd".repeat(5); // 40 characters
        assert!(SubscriberPassword::parse(long_pass).is_err());
    }

    #[test]
    fn exactly_8_characters() {
        assert!(SubscriberPassword::parse("Pass12!".to_string()).is_ok());
    }

    #[test]
    fn exactly_32_characters() {
        let pass = "P@ssw0rd".repeat(4); // 32 characters
        assert!(SubscriberPassword::parse(pass).is_ok());
    }

    #[test]
    fn no_uppercase() {
        assert!(SubscriberPassword::parse("pass123!".to_string()).is_err());
    }

    #[test]
    fn no_special_char() {
        assert!(SubscriberPassword::parse("Pass1234".to_string()).is_err());
    }
}

