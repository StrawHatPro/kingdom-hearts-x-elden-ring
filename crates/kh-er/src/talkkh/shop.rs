//! (SHOP) ELDEN RING'S MERCHANT WINDOWS -> THE KH SHOP, AND THE SMITHS' TALK REFUSED: pure rules of the talk hooks
//! (hooks.rs feeds them on the game's thread; shopkh.rs opens the KH shop and says when it closed). Numbers: the sheet
//! sora-er-shop `talk.commands` (READ in the game's own talk scripts, build/sora_tmp/er/conv/game/esd):
//!
//! - A talk script asks for the buy window with OpenRegularShop(first row, last row) (command 22, 71 scripts) and for the
//!   sell window with OpenSellShop(-1, -1) (46, 43 scripts), then waits while CheckSpecificPersonMenuIsOpen(5 / 6, 0)
//!   (query 59) reads 1, then goes back to its talk list. `shop.menu` on (default): the command is NOT RUN (Elden Ring's
//!   window never opens), a request for the KH shop is left, and query 59 for the buy / sell type (or "any", -1) reads 1
//!   for that talk until the KH shop has closed; then the game's own answer (0, nothing of its own is open) lets the
//!   script go on: the merchant's talk list comes back, the talk goes on as before.
//! - THE SMITHS (`npc.smith_talk = 0`, default): CheckActionButtonArea (query 56, the talk prompt) of a talk script that
//!   opens the reinforcement menu (the sheet's `smith.talk_ids`, generated from the game's scripts: Hewg, Iji, the
//!   Smithing Table) reads 0 WITHOUT the game's function being called: no prompt is put up, the talk never starts. A
//!   script not in the list that runs OpenEnhanceShop (command 24) all the same: the command is not run (the
//!   reinforcement menu never opens) and its talk id is refused from then on (run-time detection; logged).
//!   The other exchange windows (Enia's remembrances, Boc's alterations, Seluvis's puppets, the duplication of an Ash of
//!   War: their own commands) stay Elden Ring's.

use super::rules::{cmd, env, Decision, Val};
use kh_host::shop::TalkNumbers;

/// A merchant's request for the KH shop.
#[derive(Clone, Debug, PartialEq)]
pub struct ShopRequest {
    pub talk_id: i32,
    /// The talk object (its character: the merchant's name).
    pub ins: usize,
    /// The sell window (OpenSellShop) rather than the buy window.
    pub sell: bool,
    /// The rows the script named (OpenRegularShop's arguments; for the log).
    pub rows: (i32, i32),
    pub serial: u32,
    /// shopkh.rs opened the KH shop for it.
    pub opened: bool,
}

#[derive(Clone, Debug, Default)]
pub struct ShopTalk {
    /// `shop.menu` (default on).
    pub menu_on: bool,
    /// `npc.smith_talk = 0` (default): the smiths' talk is refused.
    pub smith_block: bool,
    /// The sheet's smiths (talk ids).
    pub smith_ids: Vec<i32>,
    /// Smiths found at run time (a script that ran OpenEnhanceShop).
    pub smith_seen: Vec<i32>,
    pub numbers: Option<TalkNumbers>,
    /// The request waiting for / shown by the KH shop (at most one).
    pub request: Option<ShopRequest>,
    serial: u32,
    /// Log lines for the frame (`shop:` lines).
    pub notes: Vec<String>,
}

impl ShopTalk {
    fn n(&self) -> TalkNumbers {
        self.numbers.unwrap_or_default()
    }

    /// A talk whose prompt is refused (a smith while `npc.smith_talk = 0`).
    pub fn refuses(&self, talk_id: i32) -> bool {
        self.smith_block && (self.smith_ids.contains(&talk_id) || self.smith_seen.contains(&talk_id))
    }

    /// A COMMAND of a talk script: `Some(Swallow)` = the game does not run it (the KH shop / nothing stands in).
    pub fn on_event(&mut self, talk_id: i32, ins: usize, id: i32, args: &[Val]) -> Option<Decision> {
        let n = self.n();
        let int = |k: usize| args.get(k).and_then(Val::int).unwrap_or(-1);
        if (id == n.open_buy || id == n.open_sell) && self.menu_on {
            self.serial = self.serial.wrapping_add(1);
            let sell = id == n.open_sell;
            self.notes.push(format!("talk {talk_id}: Elden Ring's {} window ({} {}..{}) -> the KH shop", if sell { "SELL" } else { "BUY" }, id, int(0), int(1)));
            self.request = Some(ShopRequest { talk_id, ins, sell, rows: (int(0), int(1)), serial: self.serial, opened: false });
            return Some(Decision::Swallow);
        }
        if id == n.open_enhance && self.smith_block {
            if !self.smith_seen.contains(&talk_id) && !self.smith_ids.contains(&talk_id) {
                self.smith_seen.push(talk_id);
            }
            self.notes.push(format!("talk {talk_id}: the reinforcement menu (command {id}) is NOT opened (npc.smith_talk = 0: no smithing menu yet); this talk's prompt is refused from now on"));
            return Some(Decision::Swallow);
        }
        None
    }

    /// A QUERY BEFORE THE GAME ANSWERS IT: `Some(v)` = the game's function is not called, the script reads `v`.
    pub fn before_env(&mut self, talk_id: i32, id: i32) -> Option<i32> {
        (id == self.n().action_button && self.refuses(talk_id)).then_some(0)
    }

    /// A QUERY AFTER THE GAME ANSWERED IT: `Some(v)` = the script reads `v` instead (its window reads open while the KH
    /// shop stands in for it).
    pub fn on_env(&mut self, talk_id: i32, id: i32, args: &[Val]) -> Option<i32> {
        if id != env::MENU_OPEN {
            return None;
        }
        let r = self.request.as_ref().filter(|r| r.talk_id == talk_id)?;
        let n = self.n();
        let wanted = args.first().and_then(Val::int).unwrap_or(-1);
        let ours = if r.sell { n.menu_sell } else { n.menu_buy };
        (wanted == -1 || wanted == ours).then_some(1)
    }

    /// The KH shop closed (or could not open): the script reads the game's own "closed" from now on.
    pub fn done(&mut self, why: &str) -> Option<ShopRequest> {
        let r = self.request.take()?;
        self.notes.push(format!("talk {}: the KH shop is done ({why}): the merchant's talk goes on", r.talk_id));
        Some(r)
    }
}

/// The talk list's own commands are left to rules.rs (this file never answers them).
#[allow(dead_code)]
const _LEFT_TO_RULES: [i32; 2] = [cmd::SHOW_SHOP_MESSAGE, cmd::CLOSE_MENU];

#[cfg(test)]
mod tests {
    use super::*;

    const KALE: i32 = 800_006_000;
    const HEWG: i32 = 213_006_000;

    fn talk() -> ShopTalk {
        ShopTalk { menu_on: true, smith_block: true, smith_ids: vec![HEWG], ..ShopTalk::default() }
    }

    /// (SHOP) Kalé's "Purchase": OpenRegularShop is not run, the window reads open (type 5 and "any") until the KH shop
    /// is done, then the game's own answer; his "Sell" the same with type 6; another talk's query is not touched.
    #[test]
    fn shop_the_merchants_windows_become_the_kh_shop() {
        let mut t = talk();
        assert_eq!(t.on_event(KALE, 7, 22, &[Val::Int(100500), Val::Int(100524)]), Some(Decision::Swallow));
        let r = t.request.clone().unwrap();
        assert_eq!((r.talk_id, r.sell, r.rows), (KALE, false, (100500, 100524)));
        assert_eq!(t.on_env(KALE, env::MENU_OPEN, &[Val::Int(5)]), Some(1));
        assert_eq!(t.on_env(KALE, env::MENU_OPEN, &[Val::Int(-1)]), Some(1));
        assert_eq!(t.on_env(KALE, env::MENU_OPEN, &[Val::Int(6)]), None, "the sell window is not up");
        assert_eq!(t.on_env(HEWG, env::MENU_OPEN, &[Val::Int(5)]), None, "another talk");
        assert!(t.done("closed").is_some());
        assert_eq!(t.on_env(KALE, env::MENU_OPEN, &[Val::Int(5)]), None, "closed: the game's answer");
        assert_eq!(t.on_event(KALE, 7, 46, &[Val::Int(-1), Val::Int(-1)]), Some(Decision::Swallow));
        assert!(t.request.as_ref().unwrap().sell);
        assert_eq!(t.on_env(KALE, env::MENU_OPEN, &[Val::Int(6)]), Some(1));
        // shop.menu = 0: Elden Ring's own windows
        let mut off = ShopTalk { menu_on: false, ..talk() };
        assert_eq!(off.on_event(KALE, 7, 22, &[Val::Int(100500), Val::Int(100524)]), None);
        assert!(off.request.is_none());
        // other commands are not the shop's
        assert_eq!(t.on_event(KALE, 7, cmd::SHOW_SHOP_MESSAGE, &[Val::Int(1)]), None);
    }

    /// (SHOP) THE SMITHS: their prompt query reads 0 before the game is asked; the reinforcement menu never opens; a
    /// script not in the sheet that opens it is refused from then on; `npc.smith_talk = 1` lets them talk.
    #[test]
    fn shop_the_smiths_cannot_be_talked_to() {
        let mut t = talk();
        assert_eq!(t.before_env(HEWG, 56), Some(0));
        assert_eq!(t.before_env(KALE, 56), None, "a merchant's prompt is the game's");
        assert_eq!(t.before_env(HEWG, 59), None, "only the prompt query");
        let other = 999_006_000;
        assert_eq!(t.before_env(other, 56), None);
        assert_eq!(t.on_event(other, 3, 24, &[Val::Int(0)]), Some(Decision::Swallow));
        assert_eq!(t.before_env(other, 56), Some(0), "found at run time");
        let mut open = ShopTalk { smith_block: false, ..talk() };
        assert_eq!(open.before_env(HEWG, 56), None);
        assert_eq!(open.on_event(HEWG, 3, 24, &[Val::Int(0)]), None, "npc.smith_talk = 1: the game's menu");
    }
}
