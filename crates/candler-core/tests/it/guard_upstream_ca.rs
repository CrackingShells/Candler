//! The `gateway.upstream_ca` machine setting: the user configuration hands the gateway's trust
//! anchor down, and refuses a file that cannot serve as one.

use std::fs;

use candler_core::resolve::config::{ConfigEnv, ConfigError, UserConfig};

const PEM: &str = "-----BEGIN CERTIFICATE-----\nQUJD\n-----END CERTIFICATE-----\n";

fn load(config_toml: &str, files: &[(&str, &str)]) -> Result<UserConfig, ConfigError> {
    let dir = tempfile::tempdir().expect("a temp dir");
    let xdg = dir.path().join("xdg");
    fs::create_dir_all(xdg.join("candler")).expect("create the user config dir");
    fs::write(xdg.join("candler").join("config.toml"), config_toml).expect("write the config");
    for (name, body) in files {
        fs::write(xdg.join("candler").join(name), body).expect("write a file");
    }
    UserConfig::load(&ConfigEnv::new(Some(xdg), None, None))
}

/// Guards: machine setting `gateway.upstream_ca` — unset leaves no CA (the system bundle), set
/// hands the PEM bytes of the named file (relative to the config file) to the caller; a file that
/// is missing, or holds no certificate, or a value that is no string, is refused naming the key.
/// Oracle: the bytes and keys written out by hand here.
/// Bite: drop the PEM check in `upstream_ca_key`; the `no PEM certificate` row turns red.
#[test]
fn the_upstream_ca_setting_hands_the_bytes_down_and_refuses_a_bad_file() {
    let unset = load("[slots]\ncount = 2\n", &[]).expect("a valid file");
    assert_eq!(unset.gateway_upstream_ca, None);

    let set =
        load("[gateway]\nupstream_ca = \"ca.pem\"\n", &[("ca.pem", PEM)]).expect("a valid file");
    assert_eq!(set.gateway_upstream_ca.as_deref(), Some(PEM.as_bytes()));

    let refused = [
        (
            "[gateway]\nupstream_ca = \"missing.pem\"\n",
            "cannot read the file",
        ),
        (
            "[gateway]\nupstream_ca = \"junk.pem\"\n",
            "holds no PEM certificate",
        ),
        (
            "[gateway]\nupstream_ca = \"half.pem\"\n",
            "holds no PEM certificate",
        ),
    ];
    for (toml, why) in refused {
        let files = [
            ("junk.pem", "not a certificate"),
            ("half.pem", "-----BEGIN CERTIFICATE-----\nQUJD\n"),
        ];
        let error = load(toml, &files).expect_err("a refused file");
        assert!(matches!(error, ConfigError::UpstreamCa { .. }), "{error:?}");
        let text = error.to_string();
        assert!(
            text.contains("gateway.upstream_ca") && text.contains(why),
            "{text}"
        );
    }
    let wrong = load("[gateway]\nupstream_ca = 3\n", &[]).expect_err("a wrong type");
    assert!(matches!(wrong, ConfigError::WrongType { .. }), "{wrong:?}");
}
