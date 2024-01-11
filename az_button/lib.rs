#![cfg_attr(not(feature = "std"), no_std, no_main)]

pub use self::button::ButtonRef;

#[openbrush::implementation(PSP22, PSP22Metadata, PSP22Capped)]
#[openbrush::contract]
pub mod button {
    use ink::codegen::{EmitEvent, Env};
    use openbrush::traits::Storage;

    // === EVENTS ===
    /// Event emitted when a token transfer occurs.
    #[ink(event)]
    pub struct Transfer {
        #[ink(topic)]
        from: Option<AccountId>,
        #[ink(topic)]
        to: Option<AccountId>,
        value: Balance,
    }

    /// Event emitted when an approval occurs that `spender` is allowed to withdraw
    /// up to the amount of `value` tokens from `owner`.
    #[ink(event)]
    pub struct Approval {
        #[ink(topic)]
        owner: AccountId,
        #[ink(topic)]
        spender: AccountId,
        value: Balance,
    }

    // === STRUCTS ===
    #[ink(storage)]
    #[derive(Default, Storage)]
    pub struct Button {
        #[storage_field]
        psp22: psp22::Data,
        #[storage_field]
        metadata: metadata::Data,
        #[storage_field]
        cap: capped::Data,
    }

    #[overrider(psp22::Internal)]
    fn _emit_transfer_event(
        &self,
        from: Option<AccountId>,
        to: Option<AccountId>,
        amount: Balance,
    ) {
        self.env().emit_event(Transfer {
            from,
            to,
            value: amount,
        });
    }

    #[overrider(psp22::Internal)]
    fn _emit_approval_event(&self, owner: AccountId, spender: AccountId, amount: Balance) {
        self.env().emit_event(Approval {
            owner,
            spender,
            value: amount,
        });
    }

    impl Button {
        #[ink(constructor)]
        pub fn new(
            cap: Balance,
            name: Option<String>,
            symbol: Option<String>,
            decimal: u8,
        ) -> Self {
            let mut instance = Self::default();
            assert!(capped::Internal::_init_cap(&mut instance, cap).is_ok());
            assert!(psp22::Internal::_mint_to(&mut instance, Self::env().caller(), cap).is_ok());
            instance.metadata.name.set(&name);
            instance.metadata.symbol.set(&symbol);
            instance.metadata.decimals.set(&decimal);
            instance
        }

        #[ink(message)]
        pub fn burn(&mut self, account: AccountId, amount: Balance) -> Result<(), PSP22Error> {
            let caller = Self::env().caller();
            if caller != account {
                let allowance: Balance = psp22::Internal::_allowance(self, &account, &caller);
                if allowance < amount {
                    return Err(PSP22Error::InsufficientAllowance);
                }

                psp22::Internal::_approve_from_to(self, account, caller, allowance - amount)?;
            }
            psp22::Internal::_burn_from(self, account, amount)
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
        fn init() -> (DefaultAccounts<DefaultEnvironment>, Button) {
            let accounts = default_accounts();
            set_caller::<DefaultEnvironment>(accounts.bob);
            let az_button = Button::new(
                28_000_000_000_000,
                Some("Button".to_string()),
                Some("BTN".to_string()),
                6,
            );
            (accounts, az_button)
        }

        // === TEST HANDLES ===
        #[ink::test]
        fn test_burn() {
            let (accounts, mut az_button) = init();
            // when burning from own account
            // = when balance is sufficient
            // = * it burns the amount
            az_button.burn(accounts.bob, 1_000_000_000_000).unwrap();
            let mut balance: Balance = PSP22::balance_of(&az_button, accounts.bob);
            assert_eq!(balance, 27_000_000_000_000);
            // = when balance is insufficient
            let mut result = az_button.burn(accounts.bob, 28_000_000_000_000);
            // = * it raises an error
            assert_eq!(result, Err(PSP22Error::InsufficientBalance));
            // when burning from someone else's account
            // = when balance is sufficient
            // == when allowance is insufficient
            set_caller::<DefaultEnvironment>(accounts.alice);
            // == * it raises an error
            result = az_button.burn(accounts.bob, 27_000_000_000_000);
            assert_eq!(result, Err(PSP22Error::InsufficientAllowance));
            // == when allowance is sufficient
            set_caller::<DefaultEnvironment>(accounts.bob);
            PSP22::increase_allowance(&mut az_button, accounts.alice, 28_000_000_000_000).unwrap();
            set_caller::<DefaultEnvironment>(accounts.alice);
            az_button.burn(accounts.bob, 1_000_000_000_000).unwrap();
            // == * it burns the amount
            balance = PSP22::balance_of(&az_button, accounts.bob);
            assert_eq!(balance, 26_000_000_000_000);
            // == * it decreases the allowance
            let allowance: Balance = PSP22::allowance(&az_button, accounts.bob, accounts.alice);
            assert_eq!(allowance, 27_000_000_000_000);
            // === when balance is insufficient
            result = az_button.burn(accounts.bob, 27_000_000_000_000);
            assert_eq!(result, Err(PSP22Error::InsufficientBalance));
        }
    }
}
