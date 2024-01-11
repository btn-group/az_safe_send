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
        #[ink(topic)]
        id: u32,
        #[ink(topic)]
        from: AccountId,
        #[ink(topic)]
        to: AccountId,
        amount: Balance,
        token_address: Option<AccountId>,
        fee: Balance,
    }

    #[ink(event)]
    pub struct Cancel {
        #[ink(topic)]
        id: u32,
    }

    #[ink(event)]
    pub struct Collect {
        #[ink(topic)]
        id: u32,
    }

    #[ink(event)]
    pub struct UpdateFee {
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
        #[ink(message)]
        pub fn cancel(&mut self, id: u32) -> Result<Cheque> {
            let mut cheque: Cheque = self.show(id)?;
            let caller: AccountId = Self::env().caller();
            if caller != cheque.from {
                return Err(AzSafeSendError::Unauthorised);
            }
            if cheque.status != 0 {
                return Err(AzSafeSendError::UnprocessableEntity(
                    "Status must be pending collection.".to_string(),
                ));
            }

            let mut azero_to_return_to_user: Balance = 0;
            // Return amount to caller
            if let Some(token_address_unwrapped) = cheque.token_address {
                PSP22Ref::transfer_builder(&token_address_unwrapped, caller, cheque.amount, vec![])
                    .call_flags(CallFlags::default())
                    .invoke()?;
            } else {
                azero_to_return_to_user += cheque.amount
            }

            // Return fee to caller
            azero_to_return_to_user += cheque.fee;
            if azero_to_return_to_user > 0
                && self
                    .env()
                    .transfer(caller, azero_to_return_to_user)
                    .is_err()
            {
                panic!(
                    "requested transfer failed. this can be the case if the contract does not\
                         have sufficient free funds or if the transfer would have brought the\
                         contract's balance below minimum balance."
                )
            }

            // Update cheque
            cheque.status = 2;
            self.cheques.insert(cheque.id, &cheque);

            // emit event
            Self::emit_event(self.env(), Event::Cancel(Cancel { id: cheque.id }));

            Ok(cheque)
        }

        #[ink(message)]
        pub fn collect(&mut self, id: u32) -> Result<Cheque> {
            let mut cheque: Cheque = self.show(id)?;
            let caller: AccountId = Self::env().caller();
            if caller != cheque.to {
                return Err(AzSafeSendError::Unauthorised);
            }
            if cheque.status != 0 {
                return Err(AzSafeSendError::UnprocessableEntity(
                    "Status must be pending collection.".to_string(),
                ));
            }

            if let Some(token_address_unwrapped) = cheque.token_address {
                // Transfer token to amount
                PSP22Ref::transfer_builder(&token_address_unwrapped, caller, cheque.amount, vec![])
                    .call_flags(CallFlags::default())
                    .invoke()?;
            } else if self.env().transfer(caller, cheque.amount).is_err() {
                panic!(
                    "requested transfer failed. this can be the case if the contract does not\
                             have sufficient free funds or if the transfer would have brought the\
                             contract's balance below minimum balance."
                )
            }

            // transfer fee to admin
            if cheque.fee > 0 && self.env().transfer(self.admin, cheque.fee).is_err() {
                panic!(
                    "requested transfer failed. this can be the case if the contract does not\
                             have sufficient free funds or if the transfer would have brought the\
                             contract's balance below minimum balance."
                )
            }

            // set status
            cheque.status = 1;
            self.cheques.insert(cheque.id, &cheque);

            // emit event
            Self::emit_event(self.env(), Event::Collect(Collect { id: cheque.id }));

            Ok(cheque)
        }

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

        #[ink(message)]
        pub fn update_fee(&mut self, fee: Balance) -> Result<()> {
            if Self::env().caller() != self.admin {
                return Err(AzSafeSendError::Unauthorised);
            }

            self.fee = fee;

            // emit event
            Self::emit_event(self.env(), Event::UpdateFee(UpdateFee { fee }));

            Ok(())
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

        // === CONSTANTS ===
        const MOCK_AMOUNT: Balance = 250;
        const MOCK_FEE: Balance = 500;

        // === HELPERS ===
        fn admin() -> AccountId {
            let accounts: DefaultAccounts<DefaultEnvironment> = default_accounts();
            accounts.alice
        }

        fn get_balance(account_id: AccountId) -> Balance {
            ink::env::test::get_account_balance::<ink::env::DefaultEnvironment>(account_id)
                .expect("Cannot get account balance")
        }

        fn set_balance(account_id: AccountId, balance: Balance) {
            ink::env::test::set_account_balance::<ink::env::DefaultEnvironment>(account_id, balance)
        }

        fn init() -> (DefaultAccounts<DefaultEnvironment>, AzSafeSend) {
            let accounts = default_accounts();
            set_caller::<DefaultEnvironment>(admin());
            let safe_send = AzSafeSend::new(MOCK_FEE);
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
            assert_eq!(config.fee, MOCK_FEE);
        }

        // === TEST HANDLES ===
        #[ink::test]
        fn test_cancel() {
            let (accounts, mut az_safe_send) = init();
            // when cheque doesn't exist
            let mut result = az_safe_send.cancel(0);
            // * it raises an error
            assert_eq!(result, Err(AzSafeSendError::NotFound("Cheque".to_string())));
            // when cheque exists
            ink::env::test::set_value_transferred::<ink::env::DefaultEnvironment>(
                MOCK_FEE + MOCK_AMOUNT,
            );
            let mut cheque: Cheque = az_safe_send
                .create(accounts.bob, MOCK_AMOUNT, None)
                .unwrap();
            // = when cheque doesn't belong to caller
            // = * it raises an error
            set_caller::<DefaultEnvironment>(accounts.bob);
            result = az_safe_send.cancel(0);
            assert_eq!(result, Err(AzSafeSendError::Unauthorised));
            // = when cheque belongs to caller
            set_caller::<DefaultEnvironment>(admin());
            // == when cheque is finalised
            cheque.status = 1;
            az_safe_send.cheques.insert(cheque.id, &cheque);
            // == * it raises an error
            result = az_safe_send.cancel(0);
            assert_eq!(
                result,
                Err(AzSafeSendError::UnprocessableEntity(
                    "Status must be pending collection.".to_string()
                ))
            );
            // == when cheque is cancelled
            cheque.status = 2;
            az_safe_send.cheques.insert(cheque.id, &cheque);
            // == * it raises an error
            result = az_safe_send.cancel(0);
            assert_eq!(
                result,
                Err(AzSafeSendError::UnprocessableEntity(
                    "Status must be pending collection.".to_string()
                ))
            );
            // == when cheque is pending
            cheque.status = 0;
            // === when cheque has a fee associated with it
            // ==== when cheque has a token address (TESTED BELOW IN INTEGRATION TEST)
            // ==== when cheque does not have a token address
            az_safe_send.cheques.insert(cheque.id, &cheque);
            // ===== * it sends the fee and amount back to the user
            set_balance(accounts.alice, 1_000_000);
            az_safe_send.cancel(0).unwrap();
            assert_eq!(
                get_balance(accounts.alice),
                1_000_000 + cheque.fee + cheque.amount
            );

            // === when cheque does not have a fee associated with it
            cheque.status = 0;
            cheque.fee = 0;
            az_safe_send.cheques.insert(cheque.id, &cheque);
            // ==== when cheque has a token address (TESTED BELOW IN INTEGRATION TEST)
            // ==== when cheque does not have a token address
            // ===== * it sends the fee and amount back to the user
            set_balance(accounts.alice, 1_000_000);
            az_safe_send.cancel(0).unwrap();
            assert_eq!(get_balance(accounts.alice), 1_000_000 + cheque.amount);
            // == * it sets the status to 2;
            let cheque: Cheque = az_safe_send.cheques.get(cheque.id).unwrap();
            assert_eq!(cheque.status, 2);
        }

        // This is for cheques without a token address attached to it
        #[ink::test]
        fn test_collect() {
            let (accounts, mut az_safe_send) = init();
            // when cheque doesn't exist
            let mut result = az_safe_send.collect(1);
            // * it raises an error
            assert_eq!(result, Err(AzSafeSendError::NotFound("Cheque".to_string())));
            // when cheque exists
            ink::env::test::set_value_transferred::<ink::env::DefaultEnvironment>(
                MOCK_FEE + MOCK_AMOUNT,
            );
            let mut cheque: Cheque = az_safe_send
                .create(accounts.bob, MOCK_AMOUNT, None)
                .unwrap();
            // = when cheque's to isn't the caller
            // = * it raises an error
            result = az_safe_send.collect(0);
            assert_eq!(result, Err(AzSafeSendError::Unauthorised));
            // = when cheque's to is the caller
            set_caller::<DefaultEnvironment>(accounts.bob);
            // == when cheque is collected
            cheque.status = 1;
            az_safe_send.cheques.insert(cheque.id, &cheque);
            // == * it raises an error
            result = az_safe_send.collect(0);
            assert_eq!(
                result,
                Err(AzSafeSendError::UnprocessableEntity(
                    "Status must be pending collection.".to_string()
                ))
            );
            // == when cheque is cancelled
            cheque.status = 2;
            az_safe_send.cheques.insert(cheque.id, &cheque);
            // == * it raises an error
            result = az_safe_send.collect(0);
            assert_eq!(
                result,
                Err(AzSafeSendError::UnprocessableEntity(
                    "Status must be pending collection.".to_string()
                ))
            );
            // == when cheque is pending
            cheque.status = 0;
            az_safe_send.cheques.insert(cheque.id, &cheque);
            set_balance(accounts.bob, 1_000_000);
            set_balance(accounts.alice, 1_000_000);
            result = az_safe_send.collect(0);
            let result_unwrapped = result.unwrap();
            // == * it transfers the cheque amount to the caller
            assert_eq!(get_balance(accounts.bob), 1_000_000 + cheque.amount);
            // == * it transfers the fee to the admin
            assert!(get_balance(accounts.alice) > 1_000_000);
            // == * it sets the status to 1;
            assert_eq!(result_unwrapped.status, 1);
        }

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

        #[ink::test]
        fn test_update_fee() {
            let (accounts, mut az_safe_send) = init();
            // when called by non-admin
            set_caller::<DefaultEnvironment>(accounts.bob);
            // * it raises an error
            let mut result = az_safe_send.update_fee(1);
            assert_eq!(result, Err(AzSafeSendError::Unauthorised));
            // when called by admin
            set_caller::<DefaultEnvironment>(accounts.alice);
            result = az_safe_send.update_fee(10);
            assert!(result.is_ok());
            // = * it updates the fee
            assert_eq!(az_safe_send.fee, 10);
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
        const MOCK_SEND_AMOUNT: Balance = 5;

        // === TYPES ===
        type E2EResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;

        // === HELPERS ===
        fn account_id(k: Keypair) -> AccountId {
            AccountId::try_from(k.public_key().to_account_id().as_ref())
                .expect("account keyring has a valid account id")
        }

        // === TEST HANDLES ===
        // This is just to test when cheque has a token address associated with it
        #[ink_e2e::test]
        async fn test_cancel(mut client: ::ink_e2e::Client<C, E>) -> E2EResult<()> {
            let alice_account_id: AccountId = account_id(ink_e2e::alice());
            let bob_account_id: AccountId = account_id(ink_e2e::bob());

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
            let safe_send_constructor = AzSafeSendRef::new(1_000_000_000_000);
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
            // when cheque with token address associated with it exists
            let increase_allowance_message = build_message::<ButtonRef>(token_id.clone())
                .call(|token| token.increase_allowance(safe_send_id, u128::MAX));
            client
                .call(&ink_e2e::alice(), increase_allowance_message, 0, None)
                .await
                .expect("increase allowance failed");
            let create_message =
                build_message::<AzSafeSendRef>(safe_send_id.clone()).call(|safe_send| {
                    safe_send.create(bob_account_id, MOCK_SEND_AMOUNT, Some(token_id))
                });
            client
                .call(&ink_e2e::alice(), create_message, 1_000_000_000_000, None)
                .await
                .expect("create failed");
            let before_cancel_balance: Balance = client.balance(alice_account_id).await.unwrap();
            let cancel_message = build_message::<AzSafeSendRef>(safe_send_id.clone())
                .call(|safe_send| safe_send.cancel(0));
            client
                .call(&ink_e2e::alice(), cancel_message, 0, None)
                .await
                .expect("cancel failed");
            // = it returns the fee to the creator
            let after_cancel_balance: Balance = client.balance(alice_account_id).await.unwrap();
            assert!(before_cancel_balance < after_cancel_balance);
            // = it returns the cheque amount to the user
            let balance_message = build_message::<ButtonRef>(token_id)
                .call(|token| token.balance_of(alice_account_id));
            let balance: Balance = client
                .call_dry_run(&ink_e2e::alice(), &balance_message, 0, None)
                .await
                .return_value();
            assert_eq!(balance, MOCK_AMOUNT);

            Ok(())
        }

        // The primary reason is to test that when token address is present, token is sent to the collector
        #[ink_e2e::test]
        async fn test_collect(mut client: ::ink_e2e::Client<C, E>) -> E2EResult<()> {
            let bob_account_id: AccountId = account_id(ink_e2e::bob());

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
            // = when fee is correct
            // == when user has provided allowance to contract to acquire token and has sufficient balance
            let increase_allowance_message = build_message::<ButtonRef>(token_id.clone())
                .call(|token| token.increase_allowance(safe_send_id, u128::MAX));
            client
                .call(&ink_e2e::alice(), increase_allowance_message, 0, None)
                .await
                .expect("increase allowance failed");
            let create_message =
                build_message::<AzSafeSendRef>(safe_send_id.clone()).call(|safe_send| {
                    safe_send.create(bob_account_id, MOCK_SEND_AMOUNT, Some(token_id))
                });
            client
                .call(&ink_e2e::alice(), create_message, MOCK_FEE, None)
                .await
                .expect("create failed");
            // === when called by the collector
            let collect_message = build_message::<AzSafeSendRef>(safe_send_id.clone())
                .call(|safe_send| safe_send.collect(0));
            client
                .call(&ink_e2e::bob(), collect_message, 0, None)
                .await
                .expect("create failed");
            // == * it sends the amount of token to the collector
            let balance_message = build_message::<ButtonRef>(token_id)
                .call(|button| button.balance_of(bob_account_id));
            let balance: Balance = client
                .call_dry_run(&ink_e2e::alice(), &balance_message, 0, None)
                .await
                .return_value();
            assert_eq!(MOCK_SEND_AMOUNT, balance);

            Ok(())
        }

        #[ink_e2e::test]
        async fn test_create(mut client: ::ink_e2e::Client<C, E>) -> E2EResult<()> {
            let alice_account_id: AccountId = account_id(ink_e2e::alice());
            let bob_account_id: AccountId = account_id(ink_e2e::bob());

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
            let create_message =
                build_message::<AzSafeSendRef>(safe_send_id.clone()).call(|safe_send| {
                    safe_send.create(bob_account_id, MOCK_SEND_AMOUNT, Some(token_id))
                });
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
            assert_eq!(MOCK_SEND_AMOUNT, balance);
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
            assert_eq!(cheque.amount, MOCK_SEND_AMOUNT);
            // ==== * it sets the status to 0
            assert_eq!(cheque.status, 0);
            // ==== * it stores the submitted token_address
            assert_eq!(cheque.token_address, Some(token_id));

            Ok(())
        }
    }
}
