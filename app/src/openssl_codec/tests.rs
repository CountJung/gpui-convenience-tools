use super::*;

// Public synthetic fixtures from the first 60 lines of PCF OpenSslPasswordTests.
const FIXTURES: [(&str, &str, &str); 3] = [
    (
        "U2FsdGVkX1+b5h0SvbXAQvHcO2SAg9y/LUWKhQkWDX0=",
        "123456",
        "SHA256",
    ),
    (
        "U2FsdGVkX1+NnvbajtQM9CLnkMJaRzisV1dPLi2TJW4=",
        "123456",
        "SHA256",
    ),
    (
        "U2FsdGVkX18sXxvEhaVKBm0gxGEzw/OnhVvtktdYzfQ=",
        "Md5-Pass!word",
        "MD5",
    ),
];

#[test]
fn decrypts_all_three_pcf_fixtures_and_reports_digest() {
    for (cipher, expected, digest) in FIXTURES {
        assert_eq!(
            decrypt(cipher, "911").unwrap(),
            (expected.to_owned(), digest)
        );
    }
}

#[test]
fn fixed_salt_encryption_matches_all_three_pcf_fixtures() {
    for (expected, text, digest) in FIXTURES {
        let payload = STANDARD.decode(expected).unwrap();
        let salt: [u8; 8] = payload[8..16].try_into().unwrap();
        let plaintext = format!("{text}\n");
        let actual = match digest {
            "SHA256" => encrypt_payload::<Sha256>(plaintext.as_bytes(), b"911", &salt),
            "MD5" => encrypt_payload::<Md5>(plaintext.as_bytes(), b"911", &salt),
            _ => unreachable!(),
        };
        assert_eq!(actual, expected);
    }
}

#[test]
fn decrypts_pcf_wrapped_utf8_fixture() {
    let cipher = "U2FsdGVkX1+JLQ9bnHje6pBYM1udmpsdqR4W2djU88EMslQzMfBOBwBbsxgD2e/Y\n\
                  23NEDsNv8Sv7yegBf7yfzGRM5SrS/I4GO58+P1XU/KtxtgQgvJ0Ah9+WgYbsheGG\n\
                  Fw4tuquDiB4nb7tnool5kQ==";
    assert_eq!(
        decrypt(cipher, "911").unwrap(),
        (
            "긴비밀번호-abcdefghijklmnopqrstuvwxyz-0123456789-ABCDEFGHIJKLMNOPQRSTUVWXYZ"
                .to_owned(),
            "SHA256"
        )
    );
}

#[test]
fn roundtrips_utf8_multiline_empty_and_block_boundary_inputs() {
    for text in [
        "",
        "a",
        "123456789012345",
        "1234567890123456",
        "12345678901234567",
        "한글 🔒 café\nsecond\r\nthird",
        "\0inside\0",
    ] {
        for passphrase in ["", "911", " 암호 🔑\r\n\t"] {
            for append_newline in [false, true] {
                let cipher = encrypt(text, passphrase, append_newline).unwrap();
                assert!(!cipher.contains(char::is_whitespace));
                assert_eq!(
                    decrypt(&cipher, passphrase).unwrap(),
                    (text.to_owned(), "SHA256")
                );
            }
        }
    }
}

#[test]
fn newline_option_appends_exactly_one_lf_and_encrypts_with_sha256() {
    for (text, append_newline, expected) in [
        ("text", false, "text"),
        ("text", true, "text\n"),
        ("text\n", true, "text\n\n"),
    ] {
        let cipher = encrypt(text, "911", append_newline).unwrap();
        let payload = STANDARD.decode(cipher).unwrap();
        let salt: [u8; 8] = payload[8..16].try_into().unwrap();
        assert_eq!(
            encrypt_payload::<Sha256>(expected.as_bytes(), b"911", &salt),
            STANDARD.encode(payload)
        );
    }
}

#[test]
fn strips_only_trailing_cr_lf_and_preserves_other_whitespace() {
    for text in [
        "  \tleading\ninternal\r\ntext \t",
        "  text\r\n\n\r",
        "text\n \t",
        "\r\n",
        "\u{a0}text\u{a0}",
    ] {
        for append_newline in [false, true] {
            let cipher = encrypt(text, "passphrase", append_newline).unwrap();
            assert_eq!(
                decrypt(&cipher, "passphrase").unwrap().0,
                text.trim_end_matches(['\r', '\n'])
            );
        }
    }
}

#[test]
fn accepts_wrapped_base64_and_unicode_whitespace() {
    let cipher = FIXTURES[0].0;
    let wrapped: String = cipher
        .chars()
        .flat_map(|c| [c, '\r', '\n', ' ', '\t', '\u{a0}', '\u{2003}'])
        .collect();
    assert_eq!(
        decrypt(&wrapped, "911").unwrap(),
        ("123456".to_owned(), "SHA256")
    );
}

#[test]
fn salts_are_fresh_eight_byte_os_random_values() {
    let mut salts = std::collections::HashSet::new();
    for _ in 0..24 {
        let cipher = encrypt("same plaintext", "same passphrase", true).unwrap();
        let payload = STANDARD.decode(&cipher).unwrap();
        assert_eq!(&payload[..8], HEADER);
        assert_eq!((payload.len() - 16) % 16, 0);
        assert!(salts.insert(payload[8..16].to_vec()));
        assert_eq!(
            decrypt(&cipher, "same passphrase").unwrap().0,
            "same plaintext"
        );
    }
}

#[test]
fn rejects_bad_base64_header_and_block_lengths() {
    for invalid in [
        "",
        "plain-password",
        "%%%",
        "U2FsdGVkX1==",
        "U2FsdGVkX1+b5h0SvbXAQvHcO2SAg9y/LUWKhQkWDX0",
        "U2FsdGVkX1+b5h0SvbXAQvHcO2SAg9y/LUWKhQkWDX0===",
    ] {
        assert!(decrypt(invalid, "911").is_err());
    }
    // Standard Base64 must reject noncanonical nonzero unused trailing bits.
    assert!(decrypt(&FIXTURES[0].0.replace("DX0=", "DX1="), "911").is_err());
    for size in [0, 8, 15, 16, 17, 31, 33, 47] {
        let mut payload = vec![0; size];
        if size >= 8 {
            payload[..8].copy_from_slice(HEADER);
        }
        assert!(decrypt(&STANDARD.encode(payload), "911").is_err());
    }
    let mut payload = STANDARD.decode(FIXTURES[0].0).unwrap();
    payload[0] ^= 1;
    assert!(decrypt(&STANDARD.encode(payload), "911").is_err());
}

#[test]
fn rejects_wrong_passphrase_without_including_supplied_values_in_errors() {
    for (cipher, _, _) in FIXTURES {
        let passphrase = "wrong-secret-passphrase";
        let error = decrypt(cipher, passphrase).unwrap_err();
        for message in [error.to_string(), format!("{error:?}")] {
            assert!(!message.contains(cipher));
            assert!(!message.contains(passphrase));
            assert!(!message.contains("123456"));
        }
    }
}

#[test]
fn rejects_invalid_pkcs7_padding() {
    let salt = *b"testSalt";
    let (key, iv) = derive_key_iv::<Sha256>(b"911", &salt);
    let mut inconsistent = [2; 16];
    inconsistent[14] = 1;
    for block in [[0; 16], [17; 16], inconsistent] {
        let cipher = cbc::Encryptor::<aes::Aes256>::new(&key.into(), &iv.into())
            .encrypt_padded_vec_mut::<aes::cipher::block_padding::NoPadding>(&block);
        let mut payload = HEADER.to_vec();
        payload.extend_from_slice(&salt);
        payload.extend_from_slice(&cipher);
        assert!(decrypt_payload::<Sha256>(&cipher, b"911", &salt).is_err());
        assert!(decrypt(&STANDARD.encode(payload), "911").is_err());
    }
}

#[test]
fn rejects_non_utf8_plaintext_even_when_padding_is_valid() {
    for bytes in [
        &[0xff, 0xfe][..],
        &[0xc0, 0xaf],
        &[0xed, 0xa0, 0x80],
        &[0xf0, 0x9f],
    ] {
        let salt = *b"testSalt";
        for cipher in [
            encrypt_payload::<Sha256>(bytes, b"911", &salt),
            encrypt_payload::<Md5>(bytes, b"911", &salt),
        ] {
            let error = decrypt(&cipher, "911").unwrap_err();
            assert_eq!(error.to_string(), DECRYPT_ERROR);
            assert!(!format!("{error:?}").contains("bytes:"));
        }
    }
}

#[test]
fn enforces_plaintext_and_passphrase_byte_caps_and_roundtrips_at_boundary() {
    const { assert!(MAX_INPUT_BYTES <= 64 * 1024) };
    let at_limit = "x".repeat(MAX_INPUT_BYTES);
    let over_limit = format!("{at_limit}x");
    for append_newline in [false, true] {
        let cipher = encrypt(&at_limit, &at_limit, append_newline).unwrap();
        let wrapped = cipher
            .as_bytes()
            .chunks(64)
            .map(|chunk| std::str::from_utf8(chunk).unwrap())
            .collect::<Vec<_>>()
            .join("\r\n");
        assert!(wrapped.len() <= MAX_BASE64_INPUT_BYTES);
        assert_eq!(decrypt(&wrapped, &at_limit).unwrap().0, at_limit);
        assert!(encrypt(&over_limit, "911", append_newline).is_err());
        assert!(encrypt("text", &over_limit, append_newline).is_err());
    }
    assert!(decrypt(FIXTURES[0].0, &over_limit).is_err());
    // UTF-8 bytes, not the character count, define the bound.
    let unicode_at_limit = "é".repeat(MAX_INPUT_BYTES / 2);
    assert!(encrypt(&unicode_at_limit, "911", true).is_ok());
    assert!(encrypt(&format!("{unicode_at_limit}é"), "911", true).is_err());
    assert!(encrypt("ok", &format!("{unicode_at_limit}é"), true).is_err());
}

#[test]
fn bounds_base64_before_removing_whitespace_or_decoding() {
    let cipher = FIXTURES[0].0;
    let padded = format!(
        "{cipher}{}",
        " ".repeat(MAX_BASE64_INPUT_BYTES - cipher.len())
    );
    assert_eq!(decrypt(&padded, "911").unwrap().0, "123456");
    assert!(decrypt(&format!("{padded} "), "911").is_err());
    assert!(decrypt(&" ".repeat(MAX_BASE64_INPUT_BYTES + 1), "911").is_err());
    assert!(decrypt(&"A".repeat(MAX_BASE64_INPUT_BYTES + 1), "911").is_err());
}

#[test]
fn rejects_oversize_decoded_plaintext_including_newline_only_payloads() {
    let salt = *b"testSalt";
    for plaintext in [
        vec![b'x'; MAX_INPUT_BYTES + 1],
        vec![b'\n'; MAX_INPUT_BYTES + 2],
        vec![b'x'; MAX_CIPHER_BYTES + 16],
    ] {
        let cipher = encrypt_payload::<Sha256>(&plaintext, b"911", &salt);
        assert!(cipher.len() <= MAX_BASE64_INPUT_BYTES);
        assert!(decrypt(&cipher, "911").is_err());
    }
}
