use const_format::{map_ascii_case, Case};

pub trait Feature {
    fn name(&self) -> &'static str;
}

macro_rules! feature_list {
    ($($type:ident,)*) => {
        $(
            #[derive(Default)]
            pub struct $type {}

            impl Feature for $type {
                fn name(&self) -> &'static str {
                    map_ascii_case!(Case::Kebab, stringify!($type))
                }
            }
        )*

        pub const ALL_FEATURES: &[&'static str] = &[
            $(map_ascii_case!(Case::Kebab, stringify!($type)),)*
        ];

        #[macro_export]
        macro_rules! iter_features {
            ($apply:expr) => {
                {
                    let mut v = Vec::new();
                    $(
                        let entry = $type::default();
                        v.push($apply(entry.name(), entry));
                    )*
                    v
                }
            }
        }
    }
}

#[macro_export]
macro_rules! const_for {
    ($var:ident in $iter:expr => $block:block) => {
        let mut iter = $iter;
        while let [$var, tail @ ..] = iter {
            iter = tail;
            $block
        }
    };
}

const fn contains(array: &[&str], value: &str) -> bool {
    const_for! { it in array => {
        if const_str::equal!(*it, value) {
            return true;
        }
    }}
    false
}

pub const fn validate_features(features: &[&str]) {
    const_for! { f in features => {
        assert!(contains(ALL_FEATURES, f), "Feature is not in supported list");
    }}
}

feature_list![
    BatteryCare,
    LidLogo,
    LightsAlwaysOn,
    KbdBacklight,
    Fan,
    Perf,
];
