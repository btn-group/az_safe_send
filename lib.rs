#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
mod az_safe_send {
    // === STRUCTS ===
    #[derive(Debug, Clone, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub struct Config {
        admin: AccountId,
        fee: Balance,
    }

    #[ink(storage)]
    pub struct AzSafeSend {
        fee: Balance,
        admin: AccountId,
    }
    impl AzSafeSend {
        #[ink(constructor)]
        pub fn new(fee: Balance) -> Self {
            Self {
                fee,
                admin: Self::env().caller(),
            }
        }

        // === QUERIES ===
        #[ink(message)]
        pub fn config(&self) -> Config {
            Config {
                admin: self.admin,
                fee: self.fee,
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use ink::env::{
            test::{default_accounts, set_caller, DefaultAccounts},
            DefaultEnvironment,
        };

        // === HELPERS ===
        fn admin() -> AccountId {
            let accounts: DefaultAccounts<DefaultEnvironment> = default_accounts();
            accounts.alice
        }

        fn init() -> (DefaultAccounts<DefaultEnvironment>, AzSafeSend) {
            let accounts = default_accounts();
            set_caller::<DefaultEnvironment>(admin());
            let safe_send = AzSafeSend::new(1);
            (accounts, safe_send)
        }

        // === TESTS ===

        #[ink::test]
        fn test_config() {
            let (_accounts, az_safe_send) = init();
            let config = az_safe_send.config();
            // * it returns the config
            assert_eq!(config.admin, admin());
            assert_eq!(config.fee, 1);
        }
    }
}
