/// Input validation module for security and data integrity
/// Comprehensive validation for all external inputs
use crate::types::{TokenAddress, TokenAmount, WalletAddress};
use rust_decimal::Decimal;
use serde_json::Value;
use std::str::FromStr;
use tracing::warn;

/// Validation errors
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Missing required field: {field}")]
    MissingField { field: String },

    #[error("Invalid field format: {field} - {reason}")]
    InvalidFormat { field: String, reason: String },

    #[error("Field value out of range: {field} - {reason}")]
    OutOfRange { field: String, reason: String },

    #[error("Invalid JSON structure: {reason}")]
    InvalidJson { reason: String },

    #[error("Security validation failed: {reason}")]
    SecurityViolation { reason: String },
}

/// Input validation utilities
pub struct Validator;

impl Validator {
    /// Validate wallet address input
    pub fn validate_wallet_address(input: &str) -> Result<WalletAddress, ValidationError> {
        // Check for basic format
        if input.is_empty() {
            return Err(ValidationError::MissingField {
                field: "wallet_address".to_string(),
            });
        }

        // Check for suspicious patterns
        if input.contains('\0') || input.contains('\n') || input.contains('\r') {
            return Err(ValidationError::SecurityViolation {
                reason: "Address contains null bytes or control characters".to_string(),
            });
        }

        // Validate length and format
        if !input.starts_with("0x") {
            return Err(ValidationError::InvalidFormat {
                field: "wallet_address".to_string(),
                reason: "Address must start with 0x".to_string(),
            });
        }

        if input.len() != 42 {
            return Err(ValidationError::InvalidFormat {
                field: "wallet_address".to_string(),
                reason: "Address must be exactly 42 characters (including 0x)".to_string(),
            });
        }

        // Check for valid hex characters
        let hex_part = &input[2..];
        if !hex_part.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(ValidationError::InvalidFormat {
                field: "wallet_address".to_string(),
                reason: "Address contains invalid hex characters".to_string(),
            });
        }

        WalletAddress::from_hex(input).map_err(|e| ValidationError::InvalidFormat {
            field: "wallet_address".to_string(),
            reason: format!("Failed to parse address: {}", e),
        })
    }

    /// Validate token address input
    pub fn validate_token_address(input: &str) -> Result<TokenAddress, ValidationError> {
        // Check for basic format
        if input.is_empty() {
            return Err(ValidationError::MissingField {
                field: "token_address".to_string(),
            });
        }

        // Check for suspicious patterns
        if input.contains('\0') || input.contains('\n') || input.contains('\r') {
            return Err(ValidationError::SecurityViolation {
                reason: "Token address contains null bytes or control characters".to_string(),
            });
        }

        // Handle special case for ETH
        if input.to_uppercase() == "ETH" {
            return TokenAddress::from_hex("0x0000000000000000000000000000000000000000").map_err(
                |e| ValidationError::InvalidFormat {
                    field: "token_address".to_string(),
                    reason: format!("Failed to create ETH token address: {}", e),
                },
            );
        }

        // Validate Ethereum address format
        if !input.starts_with("0x") {
            return Err(ValidationError::InvalidFormat {
                field: "token_address".to_string(),
                reason: "Token address must start with 0x or be 'ETH'".to_string(),
            });
        }

        if input.len() != 42 {
            return Err(ValidationError::InvalidFormat {
                field: "token_address".to_string(),
                reason: "Token address must be exactly 42 characters (including 0x)".to_string(),
            });
        }

        // Check for valid hex characters
        let hex_part = &input[2..];
        if !hex_part.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(ValidationError::InvalidFormat {
                field: "token_address".to_string(),
                reason: "Token address contains invalid hex characters".to_string(),
            });
        }

        TokenAddress::from_hex(input).map_err(|e| ValidationError::InvalidFormat {
            field: "token_address".to_string(),
            reason: format!("Failed to parse token address: {}", e),
        })
    }

    /// Validate token amount input
    pub fn validate_token_amount(
        amount_str: &str,
        decimals: u8,
        max_amount: Option<u64>,
    ) -> Result<TokenAmount, ValidationError> {
        if amount_str.is_empty() {
            return Err(ValidationError::MissingField {
                field: "amount".to_string(),
            });
        }

        // Check for suspicious patterns
        if amount_str.contains('\0') || amount_str.contains('\n') || amount_str.contains('\r') {
            return Err(ValidationError::SecurityViolation {
                reason: "Amount contains null bytes or control characters".to_string(),
            });
        }

        // Parse as decimal first for validation
        let decimal_amount =
            Decimal::from_str(amount_str).map_err(|e| ValidationError::InvalidFormat {
                field: "amount".to_string(),
                reason: format!("Invalid decimal format: {}", e),
            })?;

        // Check for negative amounts
        if decimal_amount.is_sign_negative() {
            return Err(ValidationError::OutOfRange {
                field: "amount".to_string(),
                reason: "Amount cannot be negative".to_string(),
            });
        }

        // Check for zero amount
        if decimal_amount.is_zero() {
            return Err(ValidationError::OutOfRange {
                field: "amount".to_string(),
                reason: "Amount cannot be zero".to_string(),
            });
        }

        // Create token amount
        let token_amount = TokenAmount::from_human_readable(amount_str, decimals).map_err(|e| {
            ValidationError::InvalidFormat {
                field: "amount".to_string(),
                reason: format!("Failed to create token amount: {}", e),
            }
        })?;

        // Check against maximum if provided
        if let Some(max) = max_amount {
            let raw_amount =
                token_amount
                    .to_raw_units()
                    .map_err(|e| ValidationError::InvalidFormat {
                        field: "amount".to_string(),
                        reason: format!("Failed to get raw units: {}", e),
                    })?;
            if raw_amount > max.into() {
                return Err(ValidationError::OutOfRange {
                    field: "amount".to_string(),
                    reason: format!("Amount {} exceeds maximum allowed {}", raw_amount, max),
                });
            }
        }

        Ok(token_amount)
    }

    /// Validate slippage tolerance
    pub fn validate_slippage_tolerance(slippage_str: &str) -> Result<Decimal, ValidationError> {
        if slippage_str.is_empty() {
            return Err(ValidationError::MissingField {
                field: "slippage_tolerance".to_string(),
            });
        }

        let slippage =
            Decimal::from_str(slippage_str).map_err(|e| ValidationError::InvalidFormat {
                field: "slippage_tolerance".to_string(),
                reason: format!("Invalid decimal format: {}", e),
            })?;

        // Check range (0.01% to 50%)
        let min_slippage = Decimal::from_str("0.0001").unwrap(); // 0.01%
        let max_slippage = Decimal::from_str("0.5").unwrap(); // 50%

        if slippage < min_slippage {
            return Err(ValidationError::OutOfRange {
                field: "slippage_tolerance".to_string(),
                reason: "Slippage tolerance must be at least 0.01%".to_string(),
            });
        }

        if slippage > max_slippage {
            return Err(ValidationError::OutOfRange {
                field: "slippage_tolerance".to_string(),
                reason: "Slippage tolerance cannot exceed 50%".to_string(),
            });
        }

        Ok(slippage)
    }

    /// Validate JSON-RPC request structure
    pub fn validate_jsonrpc_request(request: &Value) -> Result<(), ValidationError> {
        // Check for required fields
        if !request.is_object() {
            return Err(ValidationError::InvalidJson {
                reason: "Request must be a JSON object".to_string(),
            });
        }

        // Check JSON-RPC version
        match request.get("jsonrpc") {
            Some(version) => {
                if version != "2.0" {
                    return Err(ValidationError::InvalidFormat {
                        field: "jsonrpc".to_string(),
                        reason: "Must be '2.0'".to_string(),
                    });
                }
            }
            None => {
                return Err(ValidationError::MissingField {
                    field: "jsonrpc".to_string(),
                });
            }
        }

        // Check method field
        match request.get("method") {
            Some(method) => {
                if !method.is_string() {
                    return Err(ValidationError::InvalidFormat {
                        field: "method".to_string(),
                        reason: "Must be a string".to_string(),
                    });
                }

                let method_str = method.as_str().unwrap();
                if method_str.is_empty() {
                    return Err(ValidationError::InvalidFormat {
                        field: "method".to_string(),
                        reason: "Cannot be empty".to_string(),
                    });
                }

                // Check for suspicious method names
                if method_str.contains('\0')
                    || method_str.contains('\n')
                    || method_str.contains('\r')
                {
                    return Err(ValidationError::SecurityViolation {
                        reason: "Method name contains control characters".to_string(),
                    });
                }
            }
            None => {
                return Err(ValidationError::MissingField {
                    field: "method".to_string(),
                });
            }
        }

        // Validate ID if present
        if let Some(id) = request.get("id") {
            if !id.is_string() && !id.is_number() && !id.is_null() {
                return Err(ValidationError::InvalidFormat {
                    field: "id".to_string(),
                    reason: "Must be string, number, or null".to_string(),
                });
            }
        }

        Ok(())
    }

    /// Sanitize string input to prevent injection attacks
    pub fn sanitize_string(input: &str, max_length: usize) -> Result<String, ValidationError> {
        if input.len() > max_length {
            return Err(ValidationError::OutOfRange {
                field: "string_input".to_string(),
                reason: format!("Length {} exceeds maximum {}", input.len(), max_length),
            });
        }

        // Remove control characters and null bytes
        let sanitized: String = input
            .chars()
            .filter(|c| !c.is_control() && *c != '\0')
            .collect();

        if sanitized != input {
            warn!("Input sanitized: removed control characters");
        }

        Ok(sanitized)
    }

    /// Validate request size to prevent DoS
    pub fn validate_request_size(size: usize, max_size: usize) -> Result<(), ValidationError> {
        if size > max_size {
            return Err(ValidationError::SecurityViolation {
                reason: format!("Request size {} exceeds maximum {}", size, max_size),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_validate_wallet_address_valid() {
        let valid_address = "0x742d35Cc6634C0532925a3b8D8b5d0f8988Db8c7";
        let result = Validator::validate_wallet_address(valid_address);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_wallet_address_invalid_format() {
        let invalid_address = "0x742d35Cc6634C0532925a3b8D8b5d0f8988Db8c"; // Too short
        let result = Validator::validate_wallet_address(invalid_address);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_wallet_address_security_violation() {
        let malicious_address = "0x742d35Cc6634C0532925a3b8D8b5d0f8988Db8c7\0";
        let result = Validator::validate_wallet_address(malicious_address);
        assert!(matches!(
            result,
            Err(ValidationError::SecurityViolation { .. })
        ));
    }

    #[test]
    fn test_validate_token_amount_valid() {
        let result = Validator::validate_token_amount("1.5", 18, Some(10000000000000000000u64));
        if let Err(e) = &result {
            println!("Validation error: {}", e);
        }
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_token_amount_negative() {
        let result = Validator::validate_token_amount("-1.5", 18, None);
        assert!(matches!(result, Err(ValidationError::OutOfRange { .. })));
    }

    #[test]
    fn test_validate_slippage_tolerance_valid() {
        let result = Validator::validate_slippage_tolerance("0.005"); // 0.5%
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_slippage_tolerance_too_high() {
        let result = Validator::validate_slippage_tolerance("0.6"); // 60%
        assert!(matches!(result, Err(ValidationError::OutOfRange { .. })));
    }

    #[test]
    fn test_sanitize_string() {
        let input = "Hello\0World\nTest";
        let result = Validator::sanitize_string(input, 100);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "HelloWorldTest");
    }

    #[test]
    fn test_validate_request_size() {
        let result = Validator::validate_request_size(1000, 2000);
        assert!(result.is_ok());

        let result = Validator::validate_request_size(3000, 2000);
        assert!(matches!(
            result,
            Err(ValidationError::SecurityViolation { .. })
        ));
    }

    #[test]
    fn test_sanitize_string_edge_cases() {
        // Test empty string
        let result = Validator::sanitize_string("", 100);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");

        // Test string with only control characters
        let result = Validator::sanitize_string("\0\n\r\t", 100);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");

        // Test string exactly at max length
        let input = "a".repeat(50);
        let result = Validator::sanitize_string(&input, 50);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 50);

        // Test string over max length
        let input = "a".repeat(100);
        let result = Validator::sanitize_string(&input, 50);
        assert!(matches!(result, Err(ValidationError::OutOfRange { .. })));
    }

    #[test]
    fn test_validate_wallet_address_edge_cases() {
        // Test address with mixed case
        let result =
            Validator::validate_wallet_address("0x742d35Cc6634C0532925a3b8D0C9C0C8b0E4e8A0");
        assert!(result.is_ok());

        // Test address with all lowercase
        let result =
            Validator::validate_wallet_address("0x742d35cc6634c0532925a3b8d0c9c0c8b0e4e8a0");
        assert!(result.is_ok());

        // Test address with all uppercase
        let result =
            Validator::validate_wallet_address("0x742D35CC6634C0532925A3B8D0C9C0C8B0E4E8A0");
        assert!(result.is_ok());

        // Test address without 0x prefix
        let result = Validator::validate_wallet_address("742d35Cc6634C0532925a3b8D0C9C0C8b0E4e8A0");
        assert!(matches!(result, Err(ValidationError::InvalidFormat { .. })));

        // Test address with invalid characters
        let result =
            Validator::validate_wallet_address("0x742d35Cc6634C0532925a3b8D0C9C0C8b0E4e8G0");
        assert!(matches!(result, Err(ValidationError::InvalidFormat { .. })));

        // Test address too short
        let result = Validator::validate_wallet_address("0x742d35Cc6634C0532925a3b8D0C9C0C8b0E4e8");
        assert!(matches!(result, Err(ValidationError::InvalidFormat { .. })));

        // Test address too long
        let result =
            Validator::validate_wallet_address("0x742d35Cc6634C0532925a3b8D0C9C0C8b0E4e8A000");
        assert!(matches!(result, Err(ValidationError::InvalidFormat { .. })));
    }

    #[test]
    fn test_validate_token_amount_edge_cases() {
        // Test zero amount
        let result = Validator::validate_token_amount("0", 18, None);
        assert!(matches!(result, Err(ValidationError::OutOfRange { .. })));

        // Test very small positive amount
        let result = Validator::validate_token_amount("0.000000000000000001", 18, None);
        assert!(result.is_ok());

        // Test large amount
        let result = Validator::validate_token_amount("1000000000", 18, None);
        assert!(result.is_ok());

        // Test maximum reasonable amount
        let result = Validator::validate_token_amount("999999999999999999", 18, None);
        assert!(result.is_ok());

        // Test negative amount
        let result = Validator::validate_token_amount("-100", 18, None);
        assert!(matches!(result, Err(ValidationError::OutOfRange { .. })));

        // Test invalid format
        let result = Validator::validate_token_amount("not_a_number", 18, None);
        assert!(matches!(result, Err(ValidationError::InvalidFormat { .. })));

        // Test empty string
        let result = Validator::validate_token_amount("", 18, None);
        assert!(matches!(result, Err(ValidationError::MissingField { .. })));
    }

    #[test]
    fn test_validate_slippage_tolerance_edge_cases() {
        // Test minimum valid slippage
        let result = Validator::validate_slippage_tolerance("0.0001");
        assert!(result.is_ok());

        // Test maximum valid slippage
        let result = Validator::validate_slippage_tolerance("0.5");
        assert!(result.is_ok());

        // Test zero slippage
        let result = Validator::validate_slippage_tolerance("0");
        assert!(matches!(result, Err(ValidationError::OutOfRange { .. })));

        // Test negative slippage
        let result = Validator::validate_slippage_tolerance("-0.01");
        assert!(matches!(result, Err(ValidationError::OutOfRange { .. })));

        // Test slippage over 100%
        let result = Validator::validate_slippage_tolerance("1.5");
        assert!(matches!(result, Err(ValidationError::OutOfRange { .. })));

        // Test empty string
        let result = Validator::validate_slippage_tolerance("");
        assert!(matches!(result, Err(ValidationError::MissingField { .. })));

        // Test invalid format
        let result = Validator::validate_slippage_tolerance("not_a_number");
        assert!(matches!(result, Err(ValidationError::InvalidFormat { .. })));
    }

    #[test]
    fn test_validate_request_size_edge_cases() {
        // Test zero size
        let result = Validator::validate_request_size(0, 1000);
        assert!(result.is_ok());

        // Test exactly at limit
        let result = Validator::validate_request_size(1000, 1000);
        assert!(result.is_ok());

        // Test one byte over limit
        let result = Validator::validate_request_size(1001, 1000);
        assert!(matches!(
            result,
            Err(ValidationError::SecurityViolation { .. })
        ));

        // Test very large request
        let result = Validator::validate_request_size(10_000_000, 1_000_000);
        assert!(matches!(
            result,
            Err(ValidationError::SecurityViolation { .. })
        ));
    }

    #[test]
    fn test_security_validation_patterns() {
        // Test null byte injection
        let result =
            Validator::validate_wallet_address("0x742d35Cc6634C0532925a3b8D0C9C0C8b0E4e8A0\0");
        assert!(matches!(
            result,
            Err(ValidationError::SecurityViolation { .. })
        ));

        // Test newline injection
        let result =
            Validator::validate_wallet_address("0x742d35Cc6634C0532925a3b8D0C9C0C8b0E4e8A0\n");
        assert!(matches!(
            result,
            Err(ValidationError::SecurityViolation { .. })
        ));

        // Test carriage return injection
        let result =
            Validator::validate_wallet_address("0x742d35Cc6634C0532925a3b8D0C9C0C8b0E4e8A0\r");
        assert!(matches!(
            result,
            Err(ValidationError::SecurityViolation { .. })
        ));

        // Test control character injection
        let result =
            Validator::validate_wallet_address("0x742d35Cc6634C0532925a3b8D0C9C0C8b0E4e8A0\x01");
        // This should be caught by hex validation since \x01 is not a valid hex character
        assert!(matches!(result, Err(ValidationError::InvalidFormat { .. })));
    }

    #[test]
    fn test_sanitize_string_unicode() {
        // Test Unicode characters
        let result = Validator::sanitize_string("Hello 世界 🌍", 100);
        assert!(result.is_ok());
        let sanitized = result.unwrap();
        assert!(sanitized.contains("Hello"));
        assert!(sanitized.contains("世界"));
        assert!(sanitized.contains("🌍"));

        // Test mixed ASCII and Unicode with control characters
        let result = Validator::sanitize_string("Hello\0世界\n🌍\r", 100);
        assert!(result.is_ok());
        let sanitized = result.unwrap();
        assert_eq!(sanitized, "Hello世界🌍");
    }

    #[test]
    fn test_validation_error_messages() {
        // Test that error messages are informative
        let result = Validator::validate_wallet_address("invalid");
        assert!(result.is_err());
        let error = result.unwrap_err();
        match error {
            ValidationError::InvalidFormat { field, reason } => {
                assert_eq!(field, "wallet_address");
                // The actual error message is "Address must start with 0x" for "invalid"
                // since it doesn't start with 0x, not about length
                assert!(reason.contains("start with 0x"));
            }
            _ => panic!("Expected InvalidFormat error"),
        }

        // Test security violation error message
        let result =
            Validator::validate_wallet_address("0x742d35Cc6634C0532925a3b8D0C9C0C8b0E4e8A0\0");
        assert!(result.is_err());
        let error = result.unwrap_err();
        match error {
            ValidationError::SecurityViolation { reason } => {
                assert!(reason.contains("null bytes"));
            }
            _ => panic!("Expected SecurityViolation error"),
        }
    }

    #[test]
    fn test_comprehensive_input_validation() {
        // Test valid inputs
        assert!(
            Validator::validate_wallet_address("0x742d35Cc6634C0532925a3b8D0C9C0C8b0E4e8A0")
                .is_ok()
        );
        assert!(Validator::validate_token_amount("100.5", 18, None).is_ok());
        assert!(Validator::validate_slippage_tolerance("0.005").is_ok());
        assert!(Validator::sanitize_string("Normal input text", 100).is_ok());
        assert!(Validator::validate_request_size(500, 1000).is_ok());

        // Test invalid inputs
        assert!(Validator::validate_wallet_address("invalid_address").is_err());
        assert!(Validator::validate_token_amount("-100", 18, None).is_err());
        assert!(Validator::validate_slippage_tolerance("2.0").is_err());
        assert!(Validator::sanitize_string(&"x".repeat(1000), 100).is_err());
        assert!(Validator::validate_request_size(2000, 1000).is_err());
    }

    #[test]
    fn test_performance_bounds() {
        // Test that validation doesn't take too long on large inputs
        let large_string = "a".repeat(10000);
        let start = std::time::Instant::now();
        let _result = Validator::sanitize_string(&large_string, 20000);
        let duration = start.elapsed();

        // Validation should complete quickly (under 100ms for large inputs)
        assert!(duration.as_millis() < 100);
    }

    #[test]
    fn test_validate_jsonrpc_request_valid() {
        let request = json!({
            "jsonrpc": "2.0",
            "method": "tools/list",
            "id": 1
        });
        assert!(Validator::validate_jsonrpc_request(&request).is_ok());
    }

    #[test]
    fn test_validate_jsonrpc_request_missing_version() {
        let request = json!({
            "method": "tools/list",
            "id": 1
        });
        let result = Validator::validate_jsonrpc_request(&request);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_jsonrpc_request_wrong_version() {
        let request = json!({
            "jsonrpc": "1.0",
            "method": "tools/list",
            "id": 1
        });
        let result = Validator::validate_jsonrpc_request(&request);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_jsonrpc_request_missing_method() {
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1
        });
        let result = Validator::validate_jsonrpc_request(&request);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_jsonrpc_request_invalid_method_type() {
        let request = json!({
            "jsonrpc": "2.0",
            "method": 123,
            "id": 1
        });
        let result = Validator::validate_jsonrpc_request(&request);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_jsonrpc_request_empty_method() {
        let request = json!({
            "jsonrpc": "2.0",
            "method": "",
            "id": 1
        });
        let result = Validator::validate_jsonrpc_request(&request);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_jsonrpc_request_with_params() {
        let request = json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {"name": "get_balance"},
            "id": 1
        });
        assert!(Validator::validate_jsonrpc_request(&request).is_ok());
    }

    #[test]
    fn test_validate_jsonrpc_request_string_id() {
        let request = json!({
            "jsonrpc": "2.0",
            "method": "tools/list",
            "id": "request-1"
        });
        assert!(Validator::validate_jsonrpc_request(&request).is_ok());
    }

    #[test]
    fn test_validate_jsonrpc_request_null_id() {
        let request = json!({
            "jsonrpc": "2.0",
            "method": "tools/list",
            "id": null
        });
        assert!(Validator::validate_jsonrpc_request(&request).is_ok());
    }

    #[test]
    fn test_validate_token_amount_with_max() {
        // max_amount is in raw units, so for 18 decimals:
        // "0.000000000000001" = 1000 raw units
        let result = Validator::validate_token_amount("0.000000000000001", 18, Some(1000));
        assert!(result.is_ok());

        // This should exceed the max
        let result = Validator::validate_token_amount("0.000000000000002", 18, Some(1000));
        assert!(result.is_err());

        // Test with 6 decimals (like USDC)
        let result = Validator::validate_token_amount("100", 6, Some(100_000_000)); // 100 USDC
        assert!(result.is_ok());

        let result = Validator::validate_token_amount("200", 6, Some(100_000_000));
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_token_amount_control_chars() {
        let result = Validator::validate_token_amount("100\0", 18, None);
        assert!(matches!(
            result,
            Err(ValidationError::SecurityViolation { .. })
        ));

        let result = Validator::validate_token_amount("100\n", 18, None);
        assert!(matches!(
            result,
            Err(ValidationError::SecurityViolation { .. })
        ));
    }

    #[test]
    fn test_validate_token_address_eth_uppercase() {
        let result = Validator::validate_token_address("ETH");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_token_address_eth_lowercase() {
        let result = Validator::validate_token_address("eth");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_token_address_empty() {
        let result = Validator::validate_token_address("");
        assert!(matches!(result, Err(ValidationError::MissingField { .. })));
    }

    #[test]
    fn test_validate_wallet_address_empty() {
        let result = Validator::validate_wallet_address("");
        assert!(matches!(result, Err(ValidationError::MissingField { .. })));
    }
}
