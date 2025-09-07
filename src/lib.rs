#![cfg_attr(not(any(test, feature = "export-abi")), no_main)]
#![cfg_attr(not(any(test, feature = "export-abi")), no_std)]

#[macro_use]
extern crate alloc;

use alloc::vec::Vec;

use alloy_sol_types::sol;

use stylus_sdk::evm::log;
use alloy_sol_types::SolEvent;

use stylus_sdk::{alloy_primitives::U256, alloy_primitives::Address, prelude::*};

sol! {
    event MultiplyFactorUpdated(address indexed sender, uint256 multiply_factor);
    event PercentageBonusUpdated(address indexed sender, uint256 percentage_bonus);
    event OwnershipTransferred(address indexed previous_owner, address indexed new_owner);
}

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

    pub fn calculate_reward(&self, amount: U256, start_time: U256, end_time: U256, has_bonus: bool, has_strict_bonus: bool) -> U256 {
        let current_time = U256::from(self.vm().block_timestamp());
        self.calculate_reward_at_time(amount, current_time, start_time, end_time, has_bonus, has_strict_bonus)
    }

    pub fn calculate_reward_at_time(&self, amount: U256, current_time: U256, start_time: U256, end_time: U256, has_bonus: bool, has_strict_bonus: bool) -> U256 {
        let mut reward = amount;
        
        let time_decay_multiplier = if current_time <= start_time {
            self.percentage_denominator.get()

        } else if current_time >= end_time {

            self.percentage_denominator.get() / U256::from(2)
        } else {
            let total_duration = end_time - start_time;
            let elapsed_time = current_time - start_time;
            
            let max_multiplier = self.percentage_denominator.get();
            let min_multiplier = self.percentage_denominator.get() / U256::from(2); // 50%
            let decay_range = max_multiplier - min_multiplier;
            
            let decay_amount = decay_range * elapsed_time / total_duration;
            max_multiplier - decay_amount
        };
        
        reward = reward * time_decay_multiplier / self.percentage_denominator.get();
        
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

        log(MultiplyFactorUpdated {
            sender: self.vm().tx_origin(),
            multiply_factor: new_factor,
        });

        Ok(())
    }

    pub fn update_percentage_bonus(&mut self, new_bonus: U256) -> Result<(), CommonError> {
        self.assert_owner()?;
        
        if new_bonus == U256::ZERO {
            return Err(CommonError::ZeroValue(ZeroValue {}));
        }
        
        self.percentage_bonus.set(new_bonus);

        log(PercentageBonusUpdated {
            sender: self.vm().tx_origin(),
            percentage_bonus: new_bonus,
        });

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

        log(OwnershipTransferred {
            previous_owner: self.vm().tx_origin(),
            new_owner,
        });
        
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

    #[test]
    fn test_time_based_reward_decay() {
        let vm = TestVMBuilder::new()
            .sender(Address::from([0x01; 20]))
            .build();

        let mut contract = RewardProcessor::from(&vm);
        let result = contract.constructor(U256::from(5000)); // 50% strict bonus
        assert!(result.is_ok());

        let amount = U256::from(1000);
        let start_time = U256::from(1000);
        let end_time = U256::from(2000);

        let reward_at_start = contract.calculate_reward_at_time(amount, U256::from(1000), start_time, end_time, false, false);
        assert_eq!(reward_at_start, amount);

        let reward_at_middle = contract.calculate_reward_at_time(amount, U256::from(1500), start_time, end_time, false, false);
        assert_eq!(reward_at_middle, U256::from(750));

        let reward_at_end = contract.calculate_reward_at_time(amount, U256::from(2000), start_time, end_time, false, false);
        assert_eq!(reward_at_end, U256::from(500));
    }

    #[test]
    fn test_time_based_reward_with_bonuses() {
        let vm = TestVMBuilder::new()
            .sender(Address::from([0x01; 20]))
            .build();

        let mut contract = RewardProcessor::from(&vm);
        let result = contract.constructor(U256::from(5000)); // 50% strict bonus
        assert!(result.is_ok());

        let amount = U256::from(1000);
        let start_time = U256::from(1000);
        let end_time = U256::from(2000);

        let reward_with_bonuses = contract.calculate_reward_at_time(amount, U256::from(1000), start_time, end_time, true, true);
        
        let expected = U256::from(1000) + U256::from(100) + U256::from(500);
        assert_eq!(reward_with_bonuses, expected);

        let reward_middle_with_bonuses = contract.calculate_reward_at_time(amount, U256::from(1500), start_time, end_time, true, true);
        
        let expected_middle = U256::from(750) + U256::from(100) + U256::from(500);
        assert_eq!(reward_middle_with_bonuses, expected_middle);
    }

    #[test]
    fn test_time_based_reward_edge_cases() {
        let vm = TestVMBuilder::new()
            .sender(Address::from([0x01; 20]))
            .build();

        let mut contract = RewardProcessor::from(&vm);
        let result = contract.constructor(U256::from(5000));
        assert!(result.is_ok());

        let amount = U256::from(1000);
        let start_time = U256::from(1000);
        let end_time = U256::from(2000);

        let reward_before_start = contract.calculate_reward_at_time(amount, U256::from(500), start_time, end_time, false, false);
        assert_eq!(reward_before_start, amount);

        let reward_after_end = contract.calculate_reward_at_time(amount, U256::from(3000), start_time, end_time, false, false);
        assert_eq!(reward_after_end, U256::from(500));
    }
}
