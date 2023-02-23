use sodiumoxide::crypto::pwhash::argon2id13;
use base64::Engine;

#[allow(dead_code)]
pub fn make_hash_b64(passwd: &str) -> String {
    sodiumoxide::init().expect("Fatal(hash) sodiumoxide couldn't be initialised");
    let hash = argon2id13::pwhash(
        passwd.as_bytes(),
        argon2id13::OPSLIMIT_INTERACTIVE,
        argon2id13::MEMLIMIT_INTERACTIVE
    ).expect("Fatal(hash) argon2id13::pwhash failed");
    base64::engine::general_purpose::STANDARD.encode(hash.0)
}
