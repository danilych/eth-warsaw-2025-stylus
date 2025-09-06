#![cfg_attr(not(any(test, feature = "export-abi")), no_main)]
#![cfg_attr(not(any(test, feature = "export-abi")), no_std)]

#[macro_use]
extern crate alloc;

use alloc::vec::Vec;

use alloy_sol_types::sol;

use stylus_sdk::{alloy_primitives::U256, alloy_primitives::Address, prelude::*};

sol! {
    #[derive(Debug)]
    error InvalidMultiplyFactor();

    #[derive(Debug)]
    error Unauthorized();

    #[derive(Debug)]
    error ZeroValue();
}

sol_storage! {
    #[entrypoint]
    pub struct RewardProcessor {
        uint256 multiply_factor;
        address owner;
        uint256 percentage_denominator;
        uint256 percentage_bonus;
    }
}

#[derive(SolidityError, Debug)]
pub enum ConstructorError {
    InvalidMultiplyFactor(InvalidMultiplyFactor),
}

#[derive(SolidityError, Debug)]
pub enum CommonError {
    Unauthorized(Unauthorized),
    ZeroValue(ZeroValue),
    InvalidMultiplyFactor(InvalidMultiplyFactor),
}

#[public]
impl RewardProcessor {
    #[constructor]
    #[payable]
    pub fn constructor(
        &mut self,
        multiply_factor_: U256,
    ) -> Result<(), ConstructorError> {
        if multiply_factor_ == U256::ZERO {
            return Err(ConstructorError::InvalidMultiplyFactor(InvalidMultiplyFactor {}));
        }

        self.multiply_factor.set(multiply_factor_);
        self.owner.set(self.vm().tx_origin());
        self.percentage_denominator.set(U256::from(10000));
        self.percentage_bonus.set(U256::from(1000));

        Ok(())
    }

    pub fn calculate_reward(&self, amount: U256, has_bonus: bool, has_strict_bonus: bool) -> U256 {
        let mut reward = amount;
        if has_bonus {
            reward += amount * self.percentage_bonus.get() / self.percentage_denominator.get();
        }

        if has_strict_bonus {
            reward += amount * self.multiply_factor.get() / self.percentage_denominator.get();
        }

        reward
    }

    pub fn update_multiply_factor(&mut self, new_factor: U256) -> Result<(), CommonError> {
        self.assert_owner()?;
        
        if new_factor == U256::ZERO {
            return Err(CommonError::InvalidMultiplyFactor(InvalidMultiplyFactor {}));
        }
        
        self.multiply_factor.set(new_factor);
        Ok(())
    }

    pub fn assert_owner(&self) -> Result<(), CommonError> {
        if self.vm().tx_origin() != self.owner.get() {
            return Err(CommonError::Unauthorized(Unauthorized {}));
        }
        Ok(())
    }

    pub fn transfer_ownership(&mut self, new_owner: Address) -> Result<(), CommonError> {
        self.assert_owner()?;
        self.owner.set(new_owner);
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use alloy_primitives::Address;
    use stylus_sdk::testing::TestVMBuilder;

    use super::*;

    #[test]
    fn test_assert_owner() {
        let vm = TestVMBuilder::new()
            .sender(Address::from([0x01; 20]))
            .build();

        let mut contract = RewardProcessor::from(&vm);

        let result = contract.constructor(U256::from(1000000));

        assert!(result.is_ok());
        
        let owner_check = contract.assert_owner();
        assert!(owner_check.is_ok());
        
        let vm2 = TestVMBuilder::new()
            .sender(Address::from([0x02; 20]))
            .build();
        
        let contract2 = RewardProcessor::from(&vm2);
        
        let unauthorized_check = contract2.assert_owner();
        assert!(unauthorized_check.is_err());
        assert!(matches!(
            unauthorized_check.unwrap_err(),
            CommonError::Unauthorized(_)
        ));
    }

    #[test]
    fn test_update_multiply_factor() {
        let vm = TestVMBuilder::new()
            .sender(Address::from([0x01; 20]))
            .build();

        let mut contract = RewardProcessor::from(&vm);

        let result = contract.constructor(U256::from(1000000));

        assert!(result.is_ok());
        
        let owner_check = contract.assert_owner();
        assert!(owner_check.is_ok());
        
        let vm2 = TestVMBuilder::new()
            .sender(Address::from([0x02; 20]))
            .build();
        
        let mut contract2 = RewardProcessor::from(&vm2);
        
        let unauthorized_check = contract2.update_multiply_factor(U256::from(2000000));
        assert!(unauthorized_check.is_err());
        assert!(matches!(
            unauthorized_check.unwrap_err(),
            CommonError::Unauthorized(_)
        ));

        let multiply_factor_check = contract.multiply_factor.get();
        assert_eq!(multiply_factor_check, U256::from(1000000));

        let update_result = contract.update_multiply_factor(U256::from(2000000));
        assert!(update_result.is_ok());

        let multiply_factor_check2 = contract.multiply_factor.get();
        assert_eq!(multiply_factor_check2, U256::from(2000000));
    }

    #[test]
    fn test_transfer_ownership_success() {
        let vm = TestVMBuilder::new()
            .sender(Address::from([0x01; 20]))
            .build();

        let mut contract = RewardProcessor::from(&vm);

        let result = contract.constructor(U256::from(1000000));
        assert!(result.is_ok());

        let initial_owner = contract.owner.get();
        assert_eq!(initial_owner, Address::from([0x01; 20]));

        let new_owner_address = Address::from([0x03; 20]);
        let transfer_result = contract.transfer_ownership(new_owner_address);
        
        assert!(transfer_result.is_ok());
        
        let updated_owner = contract.owner.get();
        assert_eq!(updated_owner, new_owner_address);
    }

    #[test]
    fn test_transfer_ownership_unauthorized() {
        let vm = TestVMBuilder::new()
            .sender(Address::from([0x01; 20]))
            .build();

        let mut contract = RewardProcessor::from(&vm);

        let result = contract.constructor(U256::from(1000000));
        assert!(result.is_ok());

        let vm2 = TestVMBuilder::new()
            .sender(Address::from([0x02; 20]))
            .build();
        
        let mut contract2 = RewardProcessor::from(&vm2);
        
        let new_owner_address = Address::from([0x03; 20]);
        let transfer_result = contract2.transfer_ownership(new_owner_address);
        
        assert!(transfer_result.is_err());
        assert!(matches!(
            transfer_result.unwrap_err(),
            CommonError::Unauthorized(_)
        ));
        
        let owner_check = contract.owner.get();
        assert_eq!(owner_check, Address::from([0x01; 20]));
    }
}
