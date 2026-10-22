use super::*;

#[test]
fn client_credential_store_errors_are_not_misreported_as_sign_out() {
    for error in [
        "keyring: credential read failed",
        "Access is denied",
        "synthetic-secret-do-not-export",
        "",
    ] {
        let f = token_output(false, b"", error.as_bytes()).unwrap_err();
        assert_eq!(f.kind, FaultKind::Unreadable);
        assert!(!f.message.contains("synthetic-secret"));
    }
    for error in [
        "no oauth token found for github.com",
        "You are not logged into any hosts",
    ] {
        assert_eq!(
            token_output(false, b"", error.as_bytes()).unwrap_err().kind,
            FaultKind::NotSignedIn
        );
    }
}
#[test]
fn successful_client_output_remains_bounded_and_structurally_valid() {
    assert_eq!(
        token_output(true, b"synthetic-secret-token-first\r\n", b"")
            .unwrap()
            .as_str(),
        "synthetic-secret-token-first"
    );
    for bytes in [
        b"short".as_slice(),
        b"                ",
        b"synthetic bad token",
        &[b'x'; 8193],
    ] {
        assert_eq!(
            token_output(true, bytes, b"").unwrap_err().kind,
            FaultKind::Unreadable
        );
    }
}
