#![cfg_attr(not(feature = "std"), no_std, no_main)]

mod errors;

#[ink::contract]
mod az_safe_send {
    use crate::errors::AzSafeSendError;
    use ink::{
        codegen::EmitEvent, env::CallFlags, prelude::string::ToString, prelude::vec,
        reflect::ContractEventBase, storage::Mapping,
    };
    use openbrush::contracts::psp22::PSP22Ref;

    // === TYPES ===
    type Event = <AzSafeSend as ContractEventBase>::Type;
    type Result<T> = core::result::Result<T, AzSafeSendError>;

    // === EVENTS ===
    #[ink(event)]
    pub struct Create {
        id: u32,
        from: AccountId,
        to: AccountId,
        amount: Balance,
        token_address: Option<AccountId>,
        fee: Balance,
    }

    // === STRUCTS ===
    #[derive(scale::Decode, scale::Encode, Debug, Clone, PartialEq)]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout)
    )]
    pub struct Cheque {
        id: u32,
        from: AccountId,
        to: AccountId,
        amount: Balance,
        token_address: Option<AccountId>,
        status: u8,
        fee: Balance,
    }

    #[derive(Debug, Clone, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub struct Config {
        admin: AccountId,
        fee: Balance,
        cheques_total: u32,
    }

    #[ink(storage)]
    pub struct AzSafeSend {
        fee: Balance,
        admin: AccountId,
        cheques: Mapping<u32, Cheque>,
        cheques_total: u32,
    }
    impl AzSafeSend {
        #[ink(constructor)]
        pub fn new(fee: Balance) -> Self {
            Self {
                fee,
                admin: Self::env().caller(),
                cheques: Mapping::default(),
                cheques_total: 0,
            }
        }

        // === QUERIES ===
        #[ink(message)]
        pub fn config(&self) -> Config {
            Config {
                admin: self.admin,
                fee: self.fee,
                cheques_total: self.cheques_total,
            }
        }

        #[ink(message)]
        pub fn show(&self, id: u32) -> Result<Cheque> {
            if let Some(cheque) = self.cheques.get(id) {
                Ok(cheque)
            } else {
                Err(AzSafeSendError::NotFound("Cheque".to_string()))
            }
        }

        // === HANDLES ===
        // 0 == Pending Collection
        // 1 == Collected
        // 2 == Cancelled
        #[ink(message, payable)]
        pub fn create(
            &mut self,
            to: AccountId,
            amount: Balance,
            token_address: Option<AccountId>,
        ) -> Result<Cheque> {
            let caller: AccountId = Self::env().caller();
            if caller == to {
                return Err(AzSafeSendError::UnprocessableEntity(
                    "Sender and receiver must be different.".to_string(),
                ));
            }
            if amount == 0 {
                return Err(AzSafeSendError::UnprocessableEntity(
                    "Amount must be greater than zero.".to_string(),
                ));
            }
            if self.cheques_total == u32::MAX {
                return Err(AzSafeSendError::RecordsLimitReached("Cheque".to_string()));
            }
            if token_address.is_some() {
                // Check AZERO sent in equals fee if token
                if self.env().transferred_value() != self.fee {
                    return Err(AzSafeSendError::IncorrectFee);
                }

                // Transfer token from caller to contract
                self.acquire_psp22(token_address.unwrap(), caller, amount)?;
            } else {
                // Check AZERO sent in equals fee + amount if no token_address
                if self.fee.checked_add(amount).is_none()
                    || self.env().transferred_value() != self.fee + amount
                {
                    return Err(AzSafeSendError::IncorrectFee);
                }
            }

            let cheque: Cheque = Cheque {
                id: self.cheques_total,
                from: caller,
                to,
                amount,
                token_address,
                status: 0,
                fee: self.fee,
            };
            self.cheques.insert(self.cheques_total, &cheque);
            self.cheques_total += 1;

            // emit event
            Self::emit_event(
                self.env(),
                Event::Create(Create {
                    id: cheque.id,
                    from: cheque.from,
                    to: cheque.to,
                    amount: cheque.amount,
                    token_address: cheque.token_address,
                    fee: cheque.fee,
                }),
            );

            Ok(cheque)
        }

        // === PRIVATE ===
        fn acquire_psp22(&self, token: AccountId, from: AccountId, amount: Balance) -> Result<()> {
            PSP22Ref::transfer_from_builder(&token, from, self.env().account_id(), amount, vec![])
                .call_flags(CallFlags::default())
                .invoke()?;

            Ok(())
        }

        fn emit_event<EE: EmitEvent<Self>>(emitter: EE, event: Event) {
            emitter.emit_event(event);
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

        fn token_address() -> AccountId {
            let accounts: DefaultAccounts<DefaultEnvironment> = default_accounts();
            accounts.charlie
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

        // === TEST HANDLES ===
        // Testing here when token address isn't provided
        // Testing with token address in e2e tests below
        #[ink::test]
        fn test_create() {
            let (accounts, mut az_safe_send) = init();
            // when sender and receiver are the same
            // * it raises an error
            let mut result = az_safe_send.create(admin(), 1, Some(token_address()));
            assert_eq!(
                result,
                Err(AzSafeSendError::UnprocessableEntity(
                    "Sender and receiver must be different.".to_string()
                ))
            );
            // when sender and receiver are different
            // = when amount is zero
            // = * it raises an error
            result = az_safe_send.create(accounts.bob, 0, Some(token_address()));
            assert_eq!(
                result,
                Err(AzSafeSendError::UnprocessableEntity(
                    "Amount must be greater than zero.".to_string()
                ))
            );
            // == when new cheque id will be less than or equal to u32::MAX is within range
            az_safe_send.cheques_total = u32::MAX - 1;
            // === when token address is not provided
            // ==== when fee is incorrect
            let amount: Balance = 1;
            ink::env::test::set_value_transferred::<ink::env::DefaultEnvironment>(az_safe_send.fee);
            // ==== * it raises an error
            result = az_safe_send.create(accounts.bob, amount, None);
            assert_eq!(result, Err(AzSafeSendError::IncorrectFee));
            // ==== when fee is correct
            ink::env::test::set_value_transferred::<ink::env::DefaultEnvironment>(
                az_safe_send.fee + amount,
            );
            // ==== * it stores the submitter as the caller
            result = az_safe_send.create(accounts.bob, amount, None);
            let result_unwrapped = result.unwrap();
            // ==== * it increases the cheque length by 1
            assert_eq!(az_safe_send.cheques_total, u32::MAX);
            // ==== * it stores the id as the current length
            assert_eq!(result_unwrapped.id, u32::MAX - 1);
            // ==== * it stores the caller as from
            assert_eq!(result_unwrapped.from, admin());
            // ==== * it stores the to
            assert_eq!(result_unwrapped.to, accounts.bob);
            // ==== * it stores the amount
            assert_eq!(result_unwrapped.amount, amount);
            // ==== * it sets the status to 0
            assert_eq!(result_unwrapped.status, 0);
            // ==== * it stores the submitted token_address
            assert_eq!(result_unwrapped.token_address, None);
            // ==== * it stores the transaction
            assert_eq!(
                result_unwrapped,
                az_safe_send.cheques.get(result_unwrapped.id).unwrap()
            );
            // == when new cheque id will be greater than u32::MAX
            result = az_safe_send.create(accounts.bob, 1, Some(token_address()));
            assert_eq!(
                result,
                Err(AzSafeSendError::RecordsLimitReached("Cheque".to_string()))
            );
        }
    }

    #[cfg(all(test, feature = "e2e-tests"))]
    mod e2e_tests {
        use super::*;
        use crate::az_safe_send::AzSafeSendRef;
        use az_button::ButtonRef;
        use ink_e2e::{build_message, Keypair};
        use openbrush::contracts::traits::psp22::psp22_external::PSP22;

        // === CONSTANTS ===
        const MOCK_AMOUNT: Balance = 250;
        const MOCK_FEE: Balance = 500;

        // === TYPES ===
        type E2EResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;

        // === HELPERS ===
        fn account_id(k: Keypair) -> AccountId {
            AccountId::try_from(k.public_key().to_account_id().as_ref())
                .expect("account keyring has a valid account id")
        }

        // === TEST HANDLES ===
        #[ink_e2e::test]
        async fn test_create(mut client: ::ink_e2e::Client<C, E>) -> E2EResult<()> {
            let alice_account_id: AccountId = account_id(ink_e2e::alice());
            let bob_account_id: AccountId = account_id(ink_e2e::bob());
            let send_amount: Balance = 5;

            // Instantiate token
            let token_constructor = ButtonRef::new(
                MOCK_AMOUNT,
                Some("Button".to_string()),
                Some("BTN".to_string()),
                6,
            );
            let token_id: AccountId = client
                .instantiate("az_button", &ink_e2e::alice(), token_constructor, 0, None)
                .await
                .expect("Reward token instantiate failed")
                .account_id;
            // Instantiate safe send smart contract
            let safe_send_constructor = AzSafeSendRef::new(MOCK_FEE);
            let safe_send_id: AccountId = client
                .instantiate(
                    "az_safe_send",
                    &ink_e2e::alice(),
                    safe_send_constructor,
                    0,
                    None,
                )
                .await
                .expect("Safe send instantiate failed")
                .account_id;
            // when token address is supplied
            // = when fee is incorrect
            // * it raises an error
            let create_message = build_message::<AzSafeSendRef>(safe_send_id)
                .call(|safe_send| safe_send.create(bob_account_id, 1, Some(token_id)));
            let result = client
                .call_dry_run(&ink_e2e::alice(), &create_message, 0, None)
                .await
                .return_value();
            assert_eq!(result, Err(AzSafeSendError::IncorrectFee));
            // = when fee is correct
            // == when user has provided allowance to contract to acquire token and has sufficient balance
            let increase_allowance_message = build_message::<ButtonRef>(token_id.clone())
                .call(|token| token.increase_allowance(safe_send_id, u128::MAX));
            client
                .call(&ink_e2e::alice(), increase_allowance_message, 0, None)
                .await
                .expect("increase allowance failed");
            let create_message = build_message::<AzSafeSendRef>(safe_send_id.clone())
                .call(|safe_send| safe_send.create(bob_account_id, send_amount, Some(token_id)));
            client
                .call(&ink_e2e::alice(), create_message, MOCK_FEE, None)
                .await
                .expect("create failed");
            // == * it acquires the amount of token
            let balance_message =
                build_message::<ButtonRef>(token_id).call(|button| button.balance_of(safe_send_id));
            let balance: Balance = client
                .call_dry_run(&ink_e2e::alice(), &balance_message, 0, None)
                .await
                .return_value();
            assert_eq!(send_amount, balance);
            // == * it creates a new cheque with all the correct details
            let show_message =
                build_message::<AzSafeSendRef>(safe_send_id).call(|safe_send| safe_send.show(0));
            let cheque: Cheque = client
                .call_dry_run(&ink_e2e::alice(), &show_message, 0, None)
                .await
                .return_value()
                .unwrap();
            // ==== * it stores the id as the current length
            assert_eq!(cheque.id, 0);
            // ==== * it stores the caller as from
            assert_eq!(cheque.from, alice_account_id);
            // ==== * it stores the to
            assert_eq!(cheque.to, bob_account_id);
            // ==== * it stores the amount
            assert_eq!(cheque.amount, send_amount);
            // ==== * it sets the status to 0
            assert_eq!(cheque.status, 0);
            // ==== * it stores the submitted token_address
            assert_eq!(cheque.token_address, Some(token_id));

            Ok(())
        }
    }
}
