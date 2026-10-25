//! (XW-d) The XS effects' own files (`PACK/xsi/vfx/effects_xwd/`): an XS effect whose material shares its base name with
//! ANOTHER asset of the main pack reads the XS asset's own file (suffix rule `name__<last 3 folders>`), and the fragment
//! meshes get their pieces; `fx.xs_own = 0` (XS_OWN_FILES) gives the files of `effects/` as before.
//! Uses XW-d's hard-linked harness pack (build/sora_tmp/er/xwd/hpack); skipped, saying so, without it.

use std::path::PathBuf;

use kh_host::vfx::{EffectHost, XSI_NAMESPACE, XS_OWN_FILES};

fn project() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

fn materials_of(host: &mut EffectHost, effect: &str) -> Vec<String> {
    assert!(host.has_effect(effect), "{effect}");
    host.world.def(effect).unwrap().emitters.iter().map(|e| e.material.clone()).collect()
}

fn pieces_of(host: &mut EffectHost, effect: &str) -> Vec<String> {
    assert!(host.has_effect(effect), "{effect}");
    host.world.def(effect).unwrap().emitters.iter().filter(|e| e.fragments.is_some()).flat_map(|e| e.sub_meshes.iter().flat_map(|s| s.list.clone())).collect()
}

#[test]
fn xwd_the_xs_effects_read_their_own_files() {
    let pack = std::env::var_os("KH_XWD_PACK").map(PathBuf::from).unwrap_or_else(|| project().join("build/sora_tmp/er/xwd/hpack"));
    if !pack.join("xsi/vfx/effects_xwd").is_dir() {
        eprintln!("SKIPPED: no harness pack with xsi/vfx/effects_xwd at {}", pack.display());
        return;
    }
    let mut host = EffectHost::load(&pack).expect("the main effect pack");
    let line = host.load_xsi_root(&pack).expect("the XSI root");
    assert!(line.contains("9 of them read from effects_xwd/"), "{line}");
    // the main pack's own file of the XS asset (a suffixed main name): no namespace
    assert!(materials_of(&mut host, "vw_so088_fin_kira00").iter().any(|m| m == "imi_glw_01msk__wep_w_so060_mat"));
    assert!(materials_of(&mut host, "vw_so192_trf_hit0").iter().any(|m| m == "imi_glw_00k__wep_w_so060_mat"));
    // the XS root's own file under the new name: the XS table's
    assert!(materials_of(&mut host, "vw_so192_trf_fns1").iter().any(|m| m == &format!("{XSI_NAMESPACE}kmi_ora_00slk__wep_w_so190_mat")));
    assert!(materials_of(&mut host, "vw_so200_spr_tuk00").iter().any(|m| m == &format!("{XSI_NAMESPACE}zmi_sho_02slk__wep_w_so200_mat")));
    // the fragment pieces, the XS root's own meshes
    let d = pieces_of(&mut host, "vw_so088_fin_hahen03");
    assert_eq!(d.len(), 5, "{d:?}");
    assert!(d.iter().all(|m| m.starts_with(XSI_NAMESPACE) && m.contains("kmd_so088_d0")), "{d:?}");
    assert_eq!(pieces_of(&mut host, "vw_so088_fin_zmn0").len(), 18);
    for m in materials_of(&mut host, "vw_so088_fin_kira00").iter().chain(materials_of(&mut host, "vw_so192_trf_fns1").iter()) {
        assert_ne!(m, "imi_glw_01msk");
        assert_ne!(m, "kmi_ora_00slk");
    }

    // ---- fx.xs_own = 0: effects/ only, as before
    XS_OWN_FILES.store(false, std::sync::atomic::Ordering::Relaxed);
    let mut old = EffectHost::load(&pack).expect("the main effect pack");
    let line = old.load_xsi_root(&pack).expect("the XSI root");
    XS_OWN_FILES.store(true, std::sync::atomic::Ordering::Relaxed);
    assert!(!line.contains("effects_xwd"), "{line}");
    assert!(materials_of(&mut old, "vw_so088_fin_kira00").iter().any(|m| m == "imi_glw_01msk"));
    assert!(pieces_of(&mut old, "vw_so088_fin_hahen03").is_empty());
}
