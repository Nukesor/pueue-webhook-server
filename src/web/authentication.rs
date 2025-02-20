use std::collections::HashMap;

use actix_web::error::{Error, ErrorUnauthorized};
use base64::{
    Engine,
    alphabet,
    engine::{GeneralPurpose, general_purpose},
};
use hmac::{Hmac, Mac};
use sha1::Sha1;

use crate::{internal_prelude::*, settings::Settings};

type HmacSha1 = Hmac<Sha1>;

pub fn verify_authentication_header(
    settings: &Settings,
    headers: &HashMap<String, String>,
    body: &[u8],
) -> Result<(), Error> {
    // Extract the existing secret from the settings
    let secret = settings.secret.clone().unwrap_or_default();
    let has_secret = !secret.is_empty();

    // Check whether we have basic auth
    let user = settings.basic_auth_user.clone().unwrap_or_default();
    let password = settings.basic_auth_password.clone().unwrap_or_default();
    let has_basic_auth = !user.is_empty() && !password.is_empty();

    // Check whether authentication is needed and whether we need both methods for authorization to
    // work
    let authentication_required = has_basic_auth || has_secret;
    let check_both = settings.basic_auth_and_secret;

    // We don't need any authentication, return early
    if !authentication_required {
        return Ok(());
    }

    let mut signature_valid = false;

    // Check for a correct signature, if we have as secret or both authentication methods are
    // required
    if has_secret || check_both {
        let signature = get_signature_header(headers)?;
        if !signature.is_empty() {
            verify_signature_header(signature, secret, body)?;
            signature_valid = true;
        } else if check_both {
            // The signature header is required and couldn't be found
            return Err(ErrorUnauthorized("No signature header found"));
        }
    }

    // We only need one authentication method and the signature was valid
    if !check_both && signature_valid {
        return Ok(());
    }

    verify_basic_auth_header(headers, settings)?;

    Ok(())
}

/// Extract the correct signature header content from all headers
/// It's possible to receive the signature from multiple Headers, since Github uses their own
/// Header name for their signature method.
fn get_signature_header(headers: &HashMap<String, String>) -> Result<String, Error> {
    let mut header = headers.get("signature");
    if header.is_none() {
        header = headers.get("x-hub-signature");
    }

    // We dont' find any headers for signatures and this method is not required
    let mut header = if let Some(header) = header {
        header.clone()
    } else {
        return Ok("".to_string());
    };

    // Header must be formatted like this: sha1={{hash}}
    if !header.starts_with("sha1=") {
        warn!("Got request with missing sha1= prefix");
        Err(ErrorUnauthorized(
            "Error while parsing signature: Couldn't find prefix",
        ))
    } else {
        Ok(header.split_off(5))
    }
}

/// Verify the signature header. Checks our own signature generated by hmac sha1 with secret and
/// payload against the signature provided in the header.
fn verify_signature_header(signature: String, secret: String, body: &[u8]) -> Result<(), Error> {
    // Try to decode the sha1 into bytes. Should be a valid hex string
    let signature_bytes = match hex::decode(&signature) {
        Ok(result) => result,
        Err(error) => {
            warn!("Error decoding signature: {}, {}", signature, error);
            return Err(ErrorUnauthorized("Invalid sha1 signature"));
        }
    };

    // Generate the own hmac sha1 from the secret and body and verify that it's identical to the
    // signature
    let secret_bytes = secret.into_bytes();
    let expected_signature = generate_signature_sha1(&secret_bytes, body);

    match expected_signature.clone().verify_slice(&signature_bytes) {
        Ok(()) => Ok(()),
        Err(_) => {
            warn!(
                "Our sha1: {}",
                hex::encode(expected_signature.finalize().into_bytes())
            );
            warn!("Got wrong sha1: {}", signature);
            Err(ErrorUnauthorized("Invalid sha1 signature"))
        }
    }
}

/// Create a hmac SHA1 instance from a secret and body
fn generate_signature_sha1(secret_bytes: &[u8], body: &[u8]) -> HmacSha1 {
    let mut hmac =
        HmacSha1::new_from_slice(secret_bytes).expect("Couldn't create hmac with current secret");
    hmac.update(body);
    hmac
}

// Verify the basic_auth header
fn verify_basic_auth_header(
    headers: &HashMap<String, String>,
    settings: &Settings,
) -> Result<(), Error> {
    let header = headers.get("authorization");
    // Check whether we can find a Basic Auth header. It's required at this point
    let mut header = if let Some(header) = header {
        header.clone()
    } else {
        warn!("Send basic auth browser request");
        return Err(ErrorUnauthorized(""));
    };

    // Header must be formatted like this: `Basic {{base64_string}}`
    if !header.starts_with("Basic ") {
        warn!("Got request with missing basic prefix");
        return Err(ErrorUnauthorized(
            "Error while parsing signature: Couldn't find Basic prefix",
        ));
    }
    let token = header.split_off(6);

    // Decode base64 string to bytes
    let engine = GeneralPurpose::new(&alphabet::URL_SAFE, general_purpose::NO_PAD);
    let token = if let Ok(token) = engine.decode(token) {
        token
    } else {
        warn!("Got request with malformed base64");
        return Err(ErrorUnauthorized("Malformed base64"));
    };

    // Interpret bytes as UTF8
    let token = if let Ok(token) = std::str::from_utf8(&token) {
        token.to_string()
    } else {
        warn!("Got request with non utf8 token");
        return Err(ErrorUnauthorized("Invalid utf8 token"));
    };

    let credentials: Vec<&str> = token.split(':').collect();
    if credentials.len() != 2 {
        warn!("Got request with malformed credential string");
        return Err(ErrorUnauthorized("Malformed credential string"));
    }

    // Ensure user is set in config
    let user = if let Some(user) = &settings.basic_auth_user {
        user
    } else {
        return Err(ErrorUnauthorized(""));
    };

    // Ensure password is set in config
    let password = if let Some(password) = &settings.basic_auth_password {
        password
    } else {
        return Err(ErrorUnauthorized(""));
    };

    if user != credentials[0] || password != credentials[1] {
        warn!("Got invalid base64 credentials");
        return Err(ErrorUnauthorized(""));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_args() -> (Settings, HashMap<String, String>, Vec<u8>) {
        let settings = Settings {
            domain: String::new(),
            port: 8000,
            ssl_private_key: None,
            ssl_cert_chain: None,
            secret: Some("A secret string".to_string()),
            basic_auth_user: None,
            basic_auth_password: None,
            basic_auth_and_secret: false,
            webhooks: Vec::new(),
        };

        let headers = HashMap::new();

        (
            settings,
            headers,
            "{\"test\": \"A test body\"}".as_bytes().to_vec(),
        )
    }

    fn add_signature_header(
        settings: &Settings,
        headers: &mut HashMap<String, String>,
        body: &[u8],
    ) {
        let hmac = generate_signature_sha1(&settings.secret.clone().unwrap().into_bytes(), body);
        let prefix = "sha1=".to_string();
        headers.insert(
            "signature".to_string(),
            prefix + &hex::encode(hmac.finalize().into_bytes()),
        );
    }

    fn add_basic_auth_header(headers: &mut HashMap<String, String>) {
        let custom_engine = GeneralPurpose::new(&alphabet::URL_SAFE, general_purpose::NO_PAD);

        let basic_header = "TestUser:TestPassword".as_bytes();
        headers.insert(
            "authorization".to_string(),
            "Basic ".to_string() + &custom_engine.encode(basic_header),
        );
    }

    fn populate_base_auth_credentials(settings: &mut Settings) {
        settings.basic_auth_user = Some("TestUser".to_string());
        settings.basic_auth_password = Some("TestPassword".to_string());
    }

    #[test]
    /// Signature authentication should work
    fn test_valid_signature() {
        let (settings, mut headers, body) = setup_args();
        add_signature_header(&settings, &mut headers, &body);
        assert!(verify_authentication_header(&settings, &headers, &body).is_ok());
    }

    #[test]
    /// Ensure that signature authentication also works with Github's header
    fn test_valid_github_signature() {
        let (settings, mut headers, body) = setup_args();
        add_signature_header(&settings, &mut headers, &body);
        let signature = headers.remove("signature").unwrap();
        headers.insert("x-hub-signature".to_string(), signature);
        assert!(verify_authentication_header(&settings, &headers, &body).is_ok());
    }

    #[test]
    /// Requests fail if signature authentication is required, but no header is specified
    fn test_no_signature() {
        let (settings, headers, body) = setup_args();
        assert!(verify_authentication_header(&settings, &headers, &body).is_err());
    }

    #[test]
    /// Requests fail if signature authentication is required, while providing an invalid sha1
    fn test_invalid_signature() {
        let (settings, mut headers, body) = setup_args();
        headers.insert(
            "signature".to_string(),
            "sha1=a68ccdf08e2767a8307c8cda67a77f4046cb9e17".to_string(),
        );
        assert!(verify_authentication_header(&settings, &headers, &body).is_err());
    }

    #[test]
    /// Authentication fails, if both methods are required and only signature is provided
    fn test_valid_basic_auth() {
        let (mut settings, mut headers, body) = setup_args();
        populate_base_auth_credentials(&mut settings);

        add_basic_auth_header(&mut headers);
        assert!(verify_authentication_header(&settings, &headers, &body).is_ok());
    }

    #[test]
    /// Authentication fails, if basic auth is required and invalid credentials are provided
    fn test_invalid_basic_auth() {
        let (mut settings, mut headers, body) = setup_args();
        settings.secret = None;
        populate_base_auth_credentials(&mut settings);

        headers.insert(
            "authorization".to_string(),
            "Basic cm9mbDpyb2Zs".to_string(),
        );
        assert!(verify_authentication_header(&settings, &headers, &body).is_err());
    }

    #[test]
    /// Authentication works if both methods are required and provided
    fn test_both_required_working() {
        let (mut settings, mut headers, body) = setup_args();
        settings.basic_auth_and_secret = true;
        populate_base_auth_credentials(&mut settings);

        add_basic_auth_header(&mut headers);
        add_signature_header(&settings, &mut headers, &body);
        assert!(verify_authentication_header(&settings, &headers, &body).is_ok());
    }

    #[test]
    /// Authentication fails, if both methods are required and only signature is provided
    fn test_both_required_signature_provided() {
        let (mut settings, mut headers, body) = setup_args();
        settings.basic_auth_and_secret = true;
        populate_base_auth_credentials(&mut settings);

        add_signature_header(&settings, &mut headers, &body);
        assert!(verify_authentication_header(&settings, &headers, &body).is_err());
    }

    #[test]
    /// Authentication fails, if both methods are required and only basic auth is provided
    fn test_both_required_basic_auth_provided() {
        let (mut settings, mut headers, body) = setup_args();
        settings.basic_auth_and_secret = true;
        populate_base_auth_credentials(&mut settings);

        add_basic_auth_header(&mut headers);
        assert!(verify_authentication_header(&settings, &headers, &body).is_err());
    }
}
