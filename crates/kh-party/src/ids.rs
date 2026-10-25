//! Who is who: the two party members and the handles the other streams use (contract B.2).

/// A party character (`ETresChrUniqueID`; plan 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Chara {
    /// `TRES_CHR_UID_DONALD` = 11, pawn `n_ex001`.
    Donald,
    /// `TRES_CHR_UID_GOOFY` = 12, pawn `n_ex002`.
    Goofy,
}

impl Chara {
    pub const ALL: [Chara; 2] = [Chara::Donald, Chara::Goofy];

    /// `ETresChrUniqueID` value [A].
    pub const fn uid(self) -> u8 {
        match self {
            Chara::Donald => 11,
            Chara::Goofy => 12,
        }
    }

    pub const fn from_uid(uid: u8) -> Option<Chara> {
        match uid {
            11 => Some(Chara::Donald),
            12 => Some(Chara::Goofy),
            _ => None,
        }
    }

    /// The model / pawn folder name (`Character/npc/<name>`).
    pub const fn model(self) -> &'static str {
        match self {
            Chara::Donald => "n_ex001",
            Chara::Goofy => "n_ex002",
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Chara::Donald => "Donald",
            Chara::Goofy => "Goofy",
        }
    }

    /// Index of the row in `FriendRelationDataTable` (HUD slot = index + 1) [A].
    pub const fn party_index(self) -> usize {
        match self {
            Chara::Donald => 0,
            Chara::Goofy => 1,
        }
    }
}

/// A member of a [`crate::Party`]: its index in spawn order. Stable while the member exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct MemberId(pub u8);
