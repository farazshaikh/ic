use ic_crypto_ecdsa_secp256k1::{KeyDecodingError, PrivateKey, PublicKey};

#[test]
fn should_pass_wycheproof_ecdsa_secp256k1_verification_tests() -> Result<(), KeyDecodingError> {
    use wycheproof::ecdsa::*;

    let test_set =
        TestSet::load(TestName::EcdsaSecp256k1Sha256P1363).expect("Unable to load test set");

    for test_group in &test_set.test_groups {
        let pk = PublicKey::deserialize_sec1(&test_group.key.key)?;
        let pk_der = PublicKey::deserialize_der(&test_group.der)?;
        assert_eq!(pk, pk_der);

        for test in &test_group.tests {
            // The Wycheproof ECDSA tests do not normalize s so we must use
            // the verification method that accepts either valid s
            let accepted = pk.verify_signature_with_malleability(&test.msg, &test.sig);

            if accepted {
                assert_eq!(test.result, wycheproof::TestResult::Valid);
            } else if test.result != wycheproof::TestResult::Invalid {
                assert!(test.flags.contains(&TestFlag::SigSize));
            }
        }
    }

    Ok(())
}

#[test]
fn should_use_rfc6979_nonces_for_ecdsa_signature_generation() {
    // Unfortunately RFC 6979 does not include tests for secp256k1. This
    // signature was instead generated by another implementation that both supports
    // secp256k1 ECDSA and uses RFC 6979 nonce generation.

    let sk = PrivateKey::deserialize_sec1(
        &hex::decode("8f44c8e5da21a3e2933fbf732519a604891b4731f19045f078e6ce57893c1f2a")
            .expect("Valid hex"),
    )
    .expect("Valid key");

    let message = b"abc";

    let expected_sig = "d8bdb0ddfc8ebb8be42649048e92edc8547d1587b2a8f721738a2ecc0733401c70e86d3042ebbb50dccfbfbdf6c0462c7be45bcd0208d33e34efec273a86eab9";

    let generated_sig = sk.sign_message(message);

    assert_eq!(hex::encode(generated_sig), expected_sig);
}

#[test]
fn should_reject_short_x_when_deserializing_private_key() {
    for short_len in 0..31 {
        let short_x = vec![42; short_len];
        assert!(PrivateKey::deserialize_sec1(&short_x).is_err());
    }
}

#[test]
fn should_reject_long_x_when_deserializing_private_key() {
    for long_len in 33..128 {
        let long_x = vec![42; long_len];
        assert!(PrivateKey::deserialize_sec1(&long_x).is_err());
    }
}

#[test]
fn should_accept_signatures_that_we_generate() {
    use rand::RngCore;

    let mut rng = rand::thread_rng();

    let sk = PrivateKey::generate_using_rng(&mut rng);
    let pk = sk.public_key();

    for m in 0..100 {
        let mut msg = vec![0u8; m];
        rng.fill_bytes(&mut msg);
        let sig = sk.sign_message(&msg);

        assert_eq!(
            sk.sign_message(&msg),
            sig,
            "ECDSA signature generation is deterministic"
        );

        assert!(pk.verify_signature(&msg, &sig));
        assert!(pk.verify_signature_with_malleability(&msg, &sig));
    }
}

#[test]
fn should_serialization_and_deserialization_round_trip_for_private_keys(
) -> Result<(), KeyDecodingError> {
    let mut rng = rand::thread_rng();

    for _ in 0..2000 {
        let key = PrivateKey::generate_using_rng(&mut rng);

        let key_via_sec1 = PrivateKey::deserialize_sec1(&key.serialize_sec1())?;
        let key_via_p8_der = PrivateKey::deserialize_pkcs8_der(&key.serialize_pkcs8_der())?;
        let key_via_p8_pem = PrivateKey::deserialize_pkcs8_pem(&key.serialize_pkcs8_pem())?;

        let expected = key.serialize_sec1();
        assert_eq!(expected.len(), 32);

        assert_eq!(key_via_sec1.serialize_sec1(), expected);
        assert_eq!(key_via_p8_der.serialize_sec1(), expected);
        assert_eq!(key_via_p8_pem.serialize_sec1(), expected);
    }
    Ok(())
}

#[test]
fn should_serialization_and_deserialization_round_trip_for_public_keys(
) -> Result<(), KeyDecodingError> {
    let mut rng = rand::thread_rng();

    for _ in 0..2000 {
        let key = PrivateKey::generate_using_rng(&mut rng).public_key();

        let key_via_sec1 = PublicKey::deserialize_sec1(&key.serialize_sec1(false))?;
        let key_via_sec1c = PublicKey::deserialize_sec1(&key.serialize_sec1(true))?;
        let key_via_der = PublicKey::deserialize_der(&key.serialize_der())?;
        let key_via_pem = PublicKey::deserialize_pem(&key.serialize_pem())?;

        assert_eq!(key.serialize_sec1(true).len(), 33);
        let expected = key.serialize_sec1(false);
        assert_eq!(expected.len(), 65);

        assert_eq!(key_via_sec1.serialize_sec1(false), expected);
        assert_eq!(key_via_sec1c.serialize_sec1(false), expected);
        assert_eq!(key_via_der.serialize_sec1(false), expected);
        assert_eq!(key_via_pem.serialize_sec1(false), expected);
    }

    Ok(())
}

#[test]
fn should_be_able_to_parse_openssl_rfc5915_format_key() {
    pub const SAMPLE_SECP256K1_PEM: &str = r#"-----BEGIN EC PRIVATE KEY-----
MHQCAQEEIJQhkGfs2ep0VGU5BgJvcc4NVWG0GCc+aqkH7b3DL6aZoAcGBSuBBAAK
oUQDQgAENBexvaA6VKI60UxeTDHiocVBcf+y/irJOHzvQSlwiZM3MCDu6lxaP/Bw
i389XZmdlKFbsLkUI9dDQgMP98YnUA==
-----END EC PRIVATE KEY-----
"#;

    let key = PrivateKey::deserialize_rfc5915_pem(SAMPLE_SECP256K1_PEM).unwrap();

    assert_eq!(
        hex::encode(key.serialize_sec1()),
        "94219067ecd9ea7454653906026f71ce0d5561b418273e6aa907edbdc32fa699"
    );
}