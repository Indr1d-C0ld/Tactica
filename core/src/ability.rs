//! Tabella delle abilità.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Physical,
    Magic,
    Heal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Element {
    Fire,
    Ice,
    Thunder,
    Holy,
}

impl Element {
    pub fn name(self) -> &'static str {
        match self {
            Element::Fire => "fuoco",
            Element::Ice => "ghiaccio",
            Element::Thunder => "fulmine",
            Element::Holy => "luce",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AbilityId {
    Strike,
    HeavyBlow,
    Shot,
    AimedShot,
    Flame,
    Frost,
    Bolt,
    Mend,
    Radiance,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ability {
    pub id: AbilityId,
    pub name: &'static str,
    pub kind: Kind,
    pub power: i32,
    pub mp_cost: i32,
    pub range_min: i32,
    pub range_max: i32,
    /// Raggio dell'area (0 = bersaglio singolo).
    pub aoe: i32,
    /// Dislivello massimo tra chi agisce e la casella bersaglio.
    pub vertical: i32,
    pub element: Option<Element>,
    pub ranged: bool,
}

impl AbilityId {
    pub fn def(self) -> &'static Ability {
        use AbilityId as A;
        use Element as E;
        use Kind as K;
        #[allow(clippy::too_many_arguments)]
        const fn ab(
            id: AbilityId,
            name: &'static str,
            kind: Kind,
            power: i32,
            mp_cost: i32,
            range: (i32, i32),
            aoe: i32,
            vertical: i32,
            element: Option<Element>,
            ranged: bool,
        ) -> Ability {
            Ability {
                id,
                name,
                kind,
                power,
                mp_cost,
                range_min: range.0,
                range_max: range.1,
                aoe,
                vertical,
                element,
                ranged,
            }
        }
        static TABLE: [Ability; 9] = [
            ab(
                A::Strike,
                "Colpo",
                K::Physical,
                100,
                0,
                (1, 1),
                0,
                2,
                None,
                false,
            ),
            ab(
                A::HeavyBlow,
                "Fendente",
                K::Physical,
                160,
                8,
                (1, 1),
                0,
                2,
                None,
                false,
            ),
            ab(
                A::Shot,
                "Tiro",
                K::Physical,
                90,
                0,
                (2, 5),
                0,
                5,
                None,
                true,
            ),
            ab(
                A::AimedShot,
                "Tiro mirato",
                K::Physical,
                140,
                8,
                (2, 6),
                0,
                6,
                None,
                true,
            ),
            ab(
                A::Flame,
                "Fiamma",
                K::Magic,
                110,
                8,
                (0, 4),
                1,
                4,
                Some(E::Fire),
                false,
            ),
            ab(
                A::Frost,
                "Gelo",
                K::Magic,
                110,
                8,
                (0, 4),
                1,
                4,
                Some(E::Ice),
                false,
            ),
            ab(
                A::Bolt,
                "Folgore",
                K::Magic,
                160,
                14,
                (1, 4),
                0,
                6,
                Some(E::Thunder),
                false,
            ),
            ab(A::Mend, "Cura", K::Heal, 100, 6, (0, 3), 1, 4, None, false),
            ab(
                A::Radiance,
                "Luce",
                K::Magic,
                90,
                6,
                (1, 3),
                0,
                4,
                Some(E::Holy),
                false,
            ),
        ];
        &TABLE[self as usize]
    }
}
