use core::str;
use std::array::TryFromSliceError;

use crate::{
    binder::Eval,
    bls::{Compressable, SCALAR_PERIOD},
    builtin::DefaultFunction,
    constant::{self, Constant, Integer},
    data::PlutusData,
    typ::Type,
};
use bumpalo::{
    collections::{CollectIn, String as BumpString, Vec as BumpVec},
    Bump,
};
use num::{Integer as NumInteger, Signed, Zero};

use super::{cost_model, value::Value, Machine, MachineError};

pub const INTEGER_TO_BYTE_STRING_MAXIMUM_OUTPUT_LENGTH: i64 = 8192;

pub enum BuiltinSemantics {
    V1,
    V2,
}

#[derive(Debug)]
pub struct Runtime<'a, V>
where
    V: Eval<'a>,
{
    pub args: BumpVec<'a, &'a Value<'a, V>>,
    pub fun: &'a DefaultFunction,
    pub forces: usize,
}

impl<'a, V> Runtime<'a, V>
where
    V: Eval<'a>,
{
    pub fn new(arena: &'a Bump, fun: &'a DefaultFunction) -> &'a Self {
        arena.alloc(Self {
            args: BumpVec::new_in(arena),
            fun,
            forces: 0,
        })
    }

    pub fn force(&self, arena: &'a Bump) -> &'a Self {
        let new_runtime = arena.alloc(Runtime {
            args: self.args.clone(),
            fun: self.fun,
            forces: self.forces + 1,
        });

        new_runtime
    }

    pub fn push(&self, arena: &'a Bump, arg: &'a Value<'a, V>) -> &'a Self {
        let new_runtime = arena.alloc(Runtime {
            args: self.args.clone(),
            fun: self.fun,
            forces: self.forces,
        });

        new_runtime.args.push(arg);

        new_runtime
    }

    pub fn needs_force(&self) -> bool {
        self.forces < self.fun.force_count()
    }

    pub fn is_arrow(&self) -> bool {
        self.args.len() < self.fun.arity()
    }

    pub fn is_ready(&self) -> bool {
        self.args.len() == self.fun.arity()
    }
}

impl<'a> Machine<'a> {
    pub fn call<V>(
        &mut self,
        runtime: &'a Runtime<'a, V>,
    ) -> Result<&'a Value<'a, V>, MachineError<'a, V>>
    where
        V: Eval<'a>,
    {
        match runtime.fun {
            DefaultFunction::AddInteger => {
                let arg1 = runtime.args[0].unwrap_integer()?;
                let arg2 = runtime.args[1].unwrap_integer()?;

                let budget = self.costs.builtin_costs.add_integer([
                    cost_model::integer_ex_mem(arg1),
                    cost_model::integer_ex_mem(arg2),
                ]);

                self.spend_budget(budget)?;

                let result = arg1 + arg2;
                let new = self.arena.alloc(result);

                let value = Value::integer(self.arena, new);

                Ok(value)
            }
            DefaultFunction::SubtractInteger => {
                let arg1 = runtime.args[0].unwrap_integer()?;
                let arg2 = runtime.args[1].unwrap_integer()?;

                let budget = self.costs.builtin_costs.subtract_integer([
                    cost_model::integer_ex_mem(arg1),
                    cost_model::integer_ex_mem(arg2),
                ]);

                self.spend_budget(budget)?;

                let result = arg1 - arg2;

                let new = self.arena.alloc(result);

                let value = Value::integer(self.arena, new);

                Ok(value)
            }
            DefaultFunction::EqualsInteger => {
                let arg1 = runtime.args[0].unwrap_integer()?;
                let arg2 = runtime.args[1].unwrap_integer()?;

                let budget = self.costs.builtin_costs.equals_integer([
                    cost_model::integer_ex_mem(arg1),
                    cost_model::integer_ex_mem(arg2),
                ]);

                self.spend_budget(budget)?;

                let result = arg1 == arg2;

                let value = Value::bool(self.arena, result);

                Ok(value)
            }
            DefaultFunction::LessThanEqualsInteger => {
                let arg1 = runtime.args[0].unwrap_integer()?;
                let arg2 = runtime.args[1].unwrap_integer()?;

                let budget = self.costs.builtin_costs.less_than_equals_integer([
                    cost_model::integer_ex_mem(arg1),
                    cost_model::integer_ex_mem(arg2),
                ]);

                self.spend_budget(budget)?;

                let result = arg1 <= arg2;

                let value = Value::bool(self.arena, result);

                Ok(value)
            }
            DefaultFunction::AppendByteString => {
                let arg1 = runtime.args[0].unwrap_byte_string()?;
                let arg2 = runtime.args[1].unwrap_byte_string()?;

                let budget = self.costs.builtin_costs.append_byte_string([
                    cost_model::byte_string_ex_mem(arg1),
                    cost_model::byte_string_ex_mem(arg2),
                ]);

                self.spend_budget(budget)?;

                let mut result = BumpVec::with_capacity_in(arg1.len() + arg2.len(), self.arena);

                result.extend_from_slice(arg1);
                result.extend_from_slice(arg2);

                let result = self.arena.alloc(result);

                let value = Value::byte_string(self.arena, result);

                Ok(value)
            }
            DefaultFunction::EqualsByteString => {
                let arg1 = runtime.args[0].unwrap_byte_string()?;
                let arg2 = runtime.args[1].unwrap_byte_string()?;

                let budget = self.costs.builtin_costs.equals_byte_string([
                    cost_model::byte_string_ex_mem(arg1),
                    cost_model::byte_string_ex_mem(arg2),
                ]);

                self.spend_budget(budget)?;

                let result = arg1 == arg2;

                let value = Value::bool(self.arena, result);

                Ok(value)
            }
            DefaultFunction::IfThenElse => {
                let arg1 = runtime.args[0].unwrap_bool()?;
                let arg2 = runtime.args[1];
                let arg3 = runtime.args[2];

                if arg1 {
                    Ok(arg2)
                } else {
                    Ok(arg3)
                }
            }
            DefaultFunction::MultiplyInteger => {
                let arg1 = runtime.args[0].unwrap_integer()?;
                let arg2 = runtime.args[1].unwrap_integer()?;

                let budget = self.costs.builtin_costs.multiply_integer([
                    cost_model::integer_ex_mem(arg1),
                    cost_model::integer_ex_mem(arg2),
                ]);

                self.spend_budget(budget)?;

                let result = arg1 * arg2;

                let new = self.arena.alloc(result);

                let value = Value::integer(self.arena, new);

                Ok(value)
            }
            DefaultFunction::DivideInteger => {
                let arg1 = runtime.args[0].unwrap_integer()?;
                let arg2 = runtime.args[1].unwrap_integer()?;

                let budget = self.costs.builtin_costs.divide_integer([
                    cost_model::integer_ex_mem(arg1),
                    cost_model::integer_ex_mem(arg2),
                ]);

                self.spend_budget(budget)?;

                if !arg2.is_zero() {
                    let (result, _) = arg1.div_mod_floor(arg2);

                    let new = self.arena.alloc(result);

                    let value = Value::integer(self.arena, new);

                    Ok(value)
                } else {
                    Err(MachineError::division_by_zero(arg1, arg2))
                }
            }
            DefaultFunction::QuotientInteger => {
                let arg1 = runtime.args[0].unwrap_integer()?;
                let arg2 = runtime.args[1].unwrap_integer()?;

                let budget = self.costs.builtin_costs.quotient_integer([
                    cost_model::integer_ex_mem(arg1),
                    cost_model::integer_ex_mem(arg2),
                ]);

                self.spend_budget(budget)?;

                if !arg2.is_zero() {
                    let (quotient, _) = arg1.div_rem(arg2);
                    let q = self.arena.alloc(quotient);
                    let value = Value::integer(self.arena, q);
                    Ok(value)
                } else {
                    Err(MachineError::division_by_zero(arg1, arg2))
                }
            }
            DefaultFunction::RemainderInteger => {
                let arg1 = runtime.args[0].unwrap_integer()?;
                let arg2 = runtime.args[1].unwrap_integer()?;

                let budget = self.costs.builtin_costs.remainder_integer([
                    cost_model::integer_ex_mem(arg1),
                    cost_model::integer_ex_mem(arg2),
                ]);

                self.spend_budget(budget)?;

                if !arg2.is_zero() {
                    let (_, remainder) = arg1.div_rem(arg2);
                    let r = self.arena.alloc(remainder);
                    let value = Value::integer(self.arena, r);
                    Ok(value)
                } else {
                    Err(MachineError::division_by_zero(arg1, arg2))
                }
            }
            DefaultFunction::ModInteger => {
                let arg1 = runtime.args[0].unwrap_integer()?;
                let arg2 = runtime.args[1].unwrap_integer()?;

                let budget = self.costs.builtin_costs.mod_integer([
                    cost_model::integer_ex_mem(arg1),
                    cost_model::integer_ex_mem(arg2),
                ]);

                self.spend_budget(budget)?;

                if !arg2.is_zero() {
                    let (_, result) = arg1.div_mod_floor(arg2);
                    let result = self.arena.alloc(arg1 % result);
                    let value = Value::integer(self.arena, result);

                    Ok(value)
                } else {
                    Err(MachineError::division_by_zero(arg1, arg2))
                }
            }
            DefaultFunction::LessThanInteger => {
                let arg1 = runtime.args[0].unwrap_integer()?;
                let arg2 = runtime.args[1].unwrap_integer()?;

                let budget = self.costs.builtin_costs.less_than_integer([
                    cost_model::integer_ex_mem(arg1),
                    cost_model::integer_ex_mem(arg2),
                ]);

                self.spend_budget(budget)?;

                let result = arg1 < arg2;

                let value = Value::bool(self.arena, result);

                Ok(value)
            }
            DefaultFunction::ConsByteString => {
                let arg1 = runtime.args[0].unwrap_integer()?;
                let arg2 = runtime.args[1].unwrap_byte_string()?;

                let budget = self.costs.builtin_costs.cons_byte_string([
                    cost_model::integer_ex_mem(arg1),
                    cost_model::byte_string_ex_mem(arg2),
                ]);

                self.spend_budget(budget)?;

                let byte: u8 = match &self.semantics {
                    BuiltinSemantics::V1 => {
                        let wrap: Integer = arg1 % 256;

                        wrap.try_into().expect("should cast to u64 just fine")
                    }
                    BuiltinSemantics::V2 => {
                        if *arg1 > Integer::from(255) || *arg1 < Integer::from(0) {
                            return Err(MachineError::byte_string_cons_not_a_byte(arg1));
                        }

                        arg1.try_into().expect("should cast to u8 just fine")
                    }
                };

                let mut ret = BumpVec::with_capacity_in(arg2.len() + 1, self.arena);

                ret.push(byte);

                ret.extend_from_slice(arg2);

                let ret = self.arena.alloc(ret);

                let value = Value::byte_string(self.arena, ret);

                Ok(value)
            }
            DefaultFunction::SliceByteString => {
                let arg1 = runtime.args[0].unwrap_integer()?;
                let arg2: &'a num::BigInt = runtime.args[1].unwrap_integer()?;
                let arg3: &'a [u8] = runtime.args[2].unwrap_byte_string()?;

                let budget = self.costs.builtin_costs.slice_byte_string([
                    cost_model::integer_ex_mem(arg1),
                    cost_model::integer_ex_mem(arg2),
                    cost_model::byte_string_ex_mem(arg3),
                ]);

                self.spend_budget(budget)?;

                // Handle negative input case
                let skip: usize = if *arg1 < Integer::ZERO {
                    0
                } else if *arg1 >= arg3.len().into(){
                    arg3.len()
                } else {
                    arg1.try_into().expect("should cast to usize just fine")
                };

                let take: usize = if *arg2 < Integer::ZERO {
                    0
                } else if *arg2 >= arg3.len().into(){
                    arg3.len()
                } else {
                    arg2.try_into().expect("should cast to usize just fine")
                };

                let skip_take = if skip + take > arg3.len() {
                    arg3.len()
                } else {
                    skip + take
                };

                let value = Value::byte_string(self.arena, &arg3[skip..(skip_take)]);

                Ok(value)
            }
            DefaultFunction::LengthOfByteString => {
                let arg1 = runtime.args[0].unwrap_byte_string()?;

                let budget = self
                    .costs
                    .builtin_costs
                    .length_of_byte_string([cost_model::byte_string_ex_mem(arg1)]);

                self.spend_budget(budget)?;

                let result: Integer = arg1.len().into();

                let new = self.arena.alloc(result);
                let value = Value::integer(self.arena, new);

                Ok(value)
            }
            DefaultFunction::IndexByteString => {
                let arg1 = runtime.args[0].unwrap_byte_string()?;
                let arg2 = runtime.args[1].unwrap_integer()?;

                let budget = self.costs.builtin_costs.index_byte_string([
                    cost_model::byte_string_ex_mem(arg1),
                    cost_model::integer_ex_mem(arg2),
                ]);

                self.spend_budget(budget)?;

                let index: i128 = arg2.try_into().unwrap();

                if 0 <= index && (index as usize) < arg1.len() {
                    let result: Integer = arg1[index as usize].into();
                    let new = self.arena.alloc(result);
                    let value = Value::integer(self.arena, new);

                    Ok(value)
                } else {
                    Err(MachineError::byte_string_out_of_bounds(arg1, arg2))
                }
            }
            DefaultFunction::LessThanByteString => {
                let arg1 = runtime.args[0].unwrap_byte_string()?;
                let arg2 = runtime.args[1].unwrap_byte_string()?;

                let budget = self.costs.builtin_costs.less_than_byte_string([
                    cost_model::byte_string_ex_mem(arg1),
                    cost_model::byte_string_ex_mem(arg2),
                ]);

                self.spend_budget(budget)?;

                let result = arg1 < arg2;

                let value = Value::bool(self.arena, result);

                Ok(value)
            }
            DefaultFunction::LessThanEqualsByteString => {
                let arg1 = runtime.args[0].unwrap_byte_string()?;
                let arg2 = runtime.args[1].unwrap_byte_string()?;

                let budget = self.costs.builtin_costs.less_than_equals_byte_string([
                    cost_model::byte_string_ex_mem(arg1),
                    cost_model::byte_string_ex_mem(arg2),
                ]);

                self.spend_budget(budget)?;

                let result = arg1 <= arg2;

                let value = Value::bool(self.arena, result);

                Ok(value)
            }
            DefaultFunction::Sha2_256 => {
                use cryptoxide::{digest::Digest, sha2::Sha256};

                let arg1 = runtime.args[0].unwrap_byte_string()?;

                let budget = self
                    .costs
                    .builtin_costs
                    .sha2_256([cost_model::byte_string_ex_mem(arg1)]);

                self.spend_budget(budget)?;

                let mut hasher = Sha256::new();

                hasher.input(arg1);

                let mut bytes = BumpVec::with_capacity_in(hasher.output_bytes(), self.arena);

                unsafe {
                    bytes.set_len(hasher.output_bytes());
                }

                hasher.result(&mut bytes);

                let bytes = self.arena.alloc(bytes);

                let value = Value::byte_string(self.arena, bytes);

                Ok(value)
            }
            DefaultFunction::Sha3_256 => {
                use cryptoxide::{digest::Digest, sha3::Sha3_256};

                let arg1 = runtime.args[0].unwrap_byte_string()?;

                let budget = self
                    .costs
                    .builtin_costs
                    .sha3_256([cost_model::byte_string_ex_mem(arg1)]);

                self.spend_budget(budget)?;

                let mut hasher = Sha3_256::new();

                hasher.input(arg1);

                let mut bytes = BumpVec::with_capacity_in(hasher.output_bytes(), self.arena);

                unsafe {
                    bytes.set_len(hasher.output_bytes());
                }

                hasher.result(&mut bytes);

                let bytes = self.arena.alloc(bytes);

                let value = Value::byte_string(self.arena, bytes);

                Ok(value)
            }
            DefaultFunction::Blake2b_256 => {
                use cryptoxide::{blake2b::Blake2b, digest::Digest};

                let arg1 = runtime.args[0].unwrap_byte_string()?;

                let budget = self
                    .costs
                    .builtin_costs
                    .blake2b_256([cost_model::byte_string_ex_mem(arg1)]);

                self.spend_budget(budget)?;

                let mut digest = BumpVec::with_capacity_in(32, self.arena);

                unsafe {
                    digest.set_len(32);
                }

                let mut context = Blake2b::new(32);

                context.input(arg1);
                context.result(&mut digest);

                let digest = self.arena.alloc(digest);

                let value = Value::byte_string(self.arena, digest);

                Ok(value)
            }
            DefaultFunction::Keccak_256 => {
                use cryptoxide::{digest::Digest, sha3::Keccak256};

                let arg1 = runtime.args[0].unwrap_byte_string()?;

                let budget = self
                    .costs
                    .builtin_costs
                    .keccak_256([cost_model::byte_string_ex_mem(arg1)]);

                self.spend_budget(budget)?;

                let mut hasher = Keccak256::new();

                hasher.input(arg1);

                let mut bytes = BumpVec::with_capacity_in(hasher.output_bytes(), self.arena);

                unsafe {
                    bytes.set_len(hasher.output_bytes());
                }

                hasher.result(&mut bytes);

                let bytes = self.arena.alloc(bytes);

                let value = Value::byte_string(self.arena, bytes);

                Ok(value)
            }
            DefaultFunction::Blake2b_224 => {
                use cryptoxide::{blake2b::Blake2b, digest::Digest};

                let arg1 = runtime.args[0].unwrap_byte_string()?;

                let budget = self
                    .costs
                    .builtin_costs
                    .blake2b_224([cost_model::byte_string_ex_mem(arg1)]);

                self.spend_budget(budget)?;

                let mut digest = BumpVec::with_capacity_in(28, self.arena);

                unsafe {
                    digest.set_len(28);
                }

                let mut context = Blake2b::new(28);

                context.input(arg1);
                context.result(&mut digest);

                let digest = self.arena.alloc(digest);

                let value = Value::byte_string(self.arena, digest);

                Ok(value)
            }
            DefaultFunction::VerifyEd25519Signature => {
                use cryptoxide::ed25519;

                let public_key = runtime.args[0].unwrap_byte_string()?;
                let message = runtime.args[1].unwrap_byte_string()?;
                let signature = runtime.args[2].unwrap_byte_string()?;

                let budget = self.costs.builtin_costs.verify_ed25519_signature([
                    cost_model::byte_string_ex_mem(public_key),
                    cost_model::byte_string_ex_mem(message),
                    cost_model::byte_string_ex_mem(signature),
                ]);

                self.spend_budget(budget)?;

                let public_key: [u8; 32] =
                    public_key.try_into().map_err(|e: TryFromSliceError| {
                        MachineError::unexpected_ed25519_public_key_length(e)
                    })?;

                let signature: [u8; 64] =
                    signature.try_into().map_err(|e: TryFromSliceError| {
                        MachineError::unexpected_ed25519_signature_length(e)
                    })?;

                let valid = ed25519::verify(message, &public_key, &signature);

                let value = Value::bool(self.arena, valid);

                Ok(value)
            }
            DefaultFunction::VerifyEcdsaSecp256k1Signature => {
                use secp256k1::{ecdsa::Signature, Message, PublicKey, Secp256k1};

                let public_key = runtime.args[0].unwrap_byte_string()?;
                let message = runtime.args[1].unwrap_byte_string()?;
                let signature = runtime.args[2].unwrap_byte_string()?;

                let budget = self.costs.builtin_costs.verify_ecdsa_secp256k1_signature([
                    cost_model::byte_string_ex_mem(public_key),
                    cost_model::byte_string_ex_mem(message),
                    cost_model::byte_string_ex_mem(signature),
                ]);

                self.spend_budget(budget)?;

                let secp = Secp256k1::verification_only();

                let public_key =
                    PublicKey::from_slice(public_key).map_err(MachineError::secp256k1)?;

                let signature =
                    Signature::from_compact(signature).map_err(MachineError::secp256k1)?;

                let message =
                    Message::from_digest_slice(message).map_err(MachineError::secp256k1)?;

                let valid = secp.verify_ecdsa(&message, &signature, &public_key);

                let value = Value::bool(self.arena, valid.is_ok());

                Ok(value)
            }
            DefaultFunction::VerifySchnorrSecp256k1Signature => {
                use secp256k1::{schnorr::Signature, Secp256k1, XOnlyPublicKey};

                let public_key = runtime.args[0].unwrap_byte_string()?;
                let message = runtime.args[1].unwrap_byte_string()?;
                let signature = runtime.args[2].unwrap_byte_string()?;

                let budget = self
                    .costs
                    .builtin_costs
                    .verify_schnorr_secp256k1_signature([
                        cost_model::byte_string_ex_mem(public_key),
                        cost_model::byte_string_ex_mem(message),
                        cost_model::byte_string_ex_mem(signature),
                    ]);

                self.spend_budget(budget)?;

                let secp = Secp256k1::verification_only();

                let public_key =
                    XOnlyPublicKey::from_slice(public_key).map_err(MachineError::secp256k1)?;

                let signature =
                    Signature::from_slice(signature).map_err(MachineError::secp256k1)?;

                let valid = secp.verify_schnorr(&signature, message, &public_key);

                let value = Value::bool(self.arena, valid.is_ok());

                Ok(value)
            }
            DefaultFunction::AppendString => {
                let arg1 = runtime.args[0].unwrap_string()?;
                let arg2 = runtime.args[1].unwrap_string()?;

                let budget = self.costs.builtin_costs.append_string([
                    cost_model::string_ex_mem(arg1),
                    cost_model::string_ex_mem(arg2),
                ]);

                self.spend_budget(budget)?;

                let mut new = BumpString::new_in(self.arena);

                new.push_str(arg1);
                new.push_str(arg2);

                let new = self.arena.alloc(new);

                let value = Value::string(self.arena, new);

                Ok(value)
            }
            DefaultFunction::EqualsString => {
                let arg1 = runtime.args[0].unwrap_string()?;
                let arg2 = runtime.args[1].unwrap_string()?;

                let budget = self.costs.builtin_costs.equals_string([
                    cost_model::string_ex_mem(arg1),
                    cost_model::string_ex_mem(arg2),
                ]);

                self.spend_budget(budget)?;

                let value = Value::bool(self.arena, arg1 == arg2);

                Ok(value)
            }
            DefaultFunction::EncodeUtf8 => {
                let arg1 = runtime.args[0].unwrap_string()?;

                let budget = self
                    .costs
                    .builtin_costs
                    .encode_utf8([cost_model::string_ex_mem(arg1)]);

                self.spend_budget(budget)?;

                let s_bytes = arg1.as_bytes();

                let mut bytes = BumpVec::with_capacity_in(s_bytes.len(), self.arena);

                bytes.extend_from_slice(s_bytes);

                let bytes = self.arena.alloc(bytes);

                let value = Value::byte_string(self.arena, bytes);

                Ok(value)
            }
            DefaultFunction::DecodeUtf8 => {
                let arg1 = runtime.args[0].unwrap_byte_string()?;

                let budget = self
                    .costs
                    .builtin_costs
                    .decode_utf8([cost_model::byte_string_ex_mem(arg1)]);

                self.spend_budget(budget)?;

                let string = str::from_utf8(arg1).map_err(|e| MachineError::decode_utf8(e))?;

                let value = Value::string(self.arena, string);

                Ok(value)
            }
            DefaultFunction::ChooseUnit => {
                runtime.args[0].unwrap_unit()?;
                let arg2 = runtime.args[1];

                let budget = self
                    .costs
                    .builtin_costs
                    .choose_unit([cost_model::UNIT_EX_MEM, cost_model::value_ex_mem(arg2)]);

                self.spend_budget(budget)?;

                Ok(arg2)
            }
            DefaultFunction::Trace => {
                let arg1 = runtime.args[0].unwrap_string()?;
                let arg2 = runtime.args[1];

                let budget = self.costs.builtin_costs.trace([
                    cost_model::string_ex_mem(arg1),
                    cost_model::value_ex_mem(arg2),
                ]);

                self.spend_budget(budget)?;

                self.logs.push(arg1.to_string());

                Ok(arg2)
            }
            DefaultFunction::FstPair => {
                let (_, _, first, second) = runtime.args[0].unwrap_pair()?;

                let budget = self
                    .costs
                    .builtin_costs
                    .fst_pair([cost_model::pair_ex_mem(first, second)]);

                self.spend_budget(budget)?;

                let value = Value::con(self.arena, first);

                Ok(value)
            }
            DefaultFunction::SndPair => {
                let (_, _, first, second) = runtime.args[0].unwrap_pair()?;

                let budget = self
                    .costs
                    .builtin_costs
                    .snd_pair([cost_model::pair_ex_mem(first, second)]);

                self.spend_budget(budget)?;

                let value = Value::con(self.arena, second);

                Ok(value)
            }
            DefaultFunction::ChooseList => {
                let (_, list) = runtime.args[0].unwrap_list()?;
                let arg2 = runtime.args[1];
                let arg3 = runtime.args[2];

                let budget = self.costs.builtin_costs.choose_list([
                    cost_model::proto_list_ex_mem(list),
                    cost_model::value_ex_mem(arg2),
                    cost_model::value_ex_mem(arg3),
                ]);

                self.spend_budget(budget)?;

                if list.is_empty() {
                    Ok(arg2)
                } else {
                    Ok(arg3)
                }
            }
            DefaultFunction::MkCons => {
                let item = runtime.args[0].unwrap_constant()?;
                let (typ, list) = runtime.args[1].unwrap_list()?;

                let budget = self.costs.builtin_costs.mk_cons([
                    cost_model::constant_ex_mem(item),
                    cost_model::proto_list_ex_mem(list),
                ]);

                self.spend_budget(budget)?;

                if item.type_of(self.arena) != typ {
                    return Err(MachineError::mk_cons_type_mismatch(item));
                }

                let mut new_list = BumpVec::with_capacity_in(list.len() + 1, self.arena);

                new_list.push(item);

                new_list.extend_from_slice(list);

                let new_list = self.arena.alloc(new_list);

                let constant = Constant::proto_list(self.arena, typ, new_list);

                let value = constant.value(self.arena);

                Ok(value)
            }
            DefaultFunction::HeadList => {
                let (_, list) = runtime.args[0].unwrap_list()?;

                let budget = self
                    .costs
                    .builtin_costs
                    .head_list([cost_model::proto_list_ex_mem(list)]);

                self.spend_budget(budget)?;

                if list.is_empty() {
                    Err(MachineError::empty_list(list))
                } else {
                    let value = Value::con(self.arena, list[0]);

                    Ok(value)
                }
            }
            DefaultFunction::TailList => {
                let (t1, list) = runtime.args[0].unwrap_list()?;

                let budget = self
                    .costs
                    .builtin_costs
                    .tail_list([cost_model::proto_list_ex_mem(list)]);

                self.spend_budget(budget)?;

                if list.is_empty() {
                    Err(MachineError::empty_list(list))
                } else {
                    let constant = Constant::proto_list(self.arena, t1, &list[1..]);

                    let value = Value::con(self.arena, constant);

                    Ok(value)
                }
            }
            DefaultFunction::NullList => {
                let (_, list) = runtime.args[0].unwrap_list()?;

                let budget = self
                    .costs
                    .builtin_costs
                    .null_list([cost_model::proto_list_ex_mem(list)]);

                self.spend_budget(budget)?;

                let value = Value::bool(self.arena, list.is_empty());

                Ok(value)
            }
            DefaultFunction::ChooseData => {
                let arg1 = runtime.args[0].unwrap_constant()?.unwrap_data()?;
                let arg2 = runtime.args[1];
                let arg3 = runtime.args[2];
                let arg4 = runtime.args[3];
                let arg5 = runtime.args[4];
                let arg6 = runtime.args[5];

                let budget = self.costs.builtin_costs.choose_data([
                    cost_model::data_ex_mem(arg1),
                    cost_model::value_ex_mem(arg2),
                    cost_model::value_ex_mem(arg3),
                    cost_model::value_ex_mem(arg4),
                    cost_model::value_ex_mem(arg5),
                    cost_model::value_ex_mem(arg6),
                ]);

                self.spend_budget(budget)?;

                match arg1 {
                    PlutusData::Constr { .. } => Ok(arg2),
                    PlutusData::Map(_) => Ok(arg3),
                    PlutusData::List(_) => Ok(arg4),
                    PlutusData::Integer(_) => Ok(arg5),
                    PlutusData::ByteString(_) => Ok(arg6),
                }
            }
            DefaultFunction::ConstrData => {
                let tag = runtime.args[0].unwrap_integer()?;
                let (typ, fields) = runtime.args[1].unwrap_list()?;

                let budget = self.costs.builtin_costs.constr_data([
                    cost_model::integer_ex_mem(tag),
                    cost_model::proto_list_ex_mem(fields),
                ]);

                self.spend_budget(budget)?;

                if *typ != Type::Data {
                    return Err(MachineError::type_mismatch(
                        Type::Data,
                        runtime.args[1].unwrap_constant()?,
                    ));
                }

                let tag = tag.try_into().expect("should cast to u64 just fine");
                let fields: BumpVec<'_, _> = fields
                    .iter()
                    .map(|d| match d {
                        Constant::Data(d) => *d,
                        _ => unreachable!(),
                    })
                    .collect_in(self.arena);
                let fields = self.arena.alloc(fields);

                let data = PlutusData::constr(self.arena, tag, fields);

                let constant = Constant::data(self.arena, data);

                let value = Value::con(self.arena, constant);

                Ok(value)
            }
            DefaultFunction::MapData => {
                let (r#type, list) = runtime.args[0].unwrap_list()?;

                if !matches!(r#type, Type::Pair(Type::Data, Type::Data)) {
                    return Err(MachineError::type_mismatch(
                        Type::List(Type::pair(
                            self.arena,
                            Type::data(self.arena),
                            Type::data(self.arena),
                        )),
                        runtime.args[0].unwrap_constant()?,
                    ));
                }

                let budget = self
                    .costs
                    .builtin_costs
                    .map_data([cost_model::proto_list_ex_mem(list)]);

                self.spend_budget(budget)?;

                let mut map = BumpVec::new_in(self.arena);

                for item in list {
                    let Constant::ProtoPair(Type::Data, Type::Data, left, right) = item else {
                        unreachable!("is this really unreachable?")
                    };

                    let Constant::Data(key) = left else {
                        unreachable!()
                    };

                    let Constant::Data(value) = right else {
                        unreachable!()
                    };

                    map.push((*key, *value));
                }

                let map = self.arena.alloc(map);

                let value = PlutusData::map(self.arena, map)
                    .constant(self.arena)
                    .value(self.arena);

                Ok(value)
            }
            DefaultFunction::ListData => {
                let (typ, fields) = runtime.args[0].unwrap_list()?;

                let budget = self
                    .costs
                    .builtin_costs
                    .list_data([cost_model::proto_list_ex_mem(fields)]);

                self.spend_budget(budget)?;

                if *typ != Type::Data {
                    return Err(MachineError::type_mismatch(
                        Type::Data,
                        runtime.args[0].unwrap_constant()?,
                    ));
                }

                let fields: BumpVec<'_, _> = fields
                    .iter()
                    .map(|d| match d {
                        Constant::Data(d) => *d,
                        _ => unreachable!(),
                    })
                    .collect_in(self.arena);
                let fields = self.arena.alloc(fields);

                let value = PlutusData::list(self.arena, fields)
                    .constant(self.arena)
                    .value(self.arena);

                Ok(value)
            }
            DefaultFunction::IData => {
                let i = runtime.args[0].unwrap_integer()?;

                let budget = self
                    .costs
                    .builtin_costs
                    .i_data([cost_model::integer_ex_mem(i)]);

                self.spend_budget(budget)?;

                let i = PlutusData::integer(self.arena, i);

                let value = i.constant(self.arena).value(self.arena);

                Ok(value)
            }
            DefaultFunction::BData => {
                let b = runtime.args[0].unwrap_byte_string()?;

                let budget = self
                    .costs
                    .builtin_costs
                    .b_data([cost_model::byte_string_ex_mem(b)]);

                self.spend_budget(budget)?;

                let b = PlutusData::byte_string(self.arena, b);

                let value = b.constant(self.arena).value(self.arena);

                Ok(value)
            }
            DefaultFunction::UnConstrData => {
                let (tag, fields) = runtime.args[0]
                    .unwrap_constant()?
                    .unwrap_data()?
                    .unwrap_constr()?;

                let budget = self
                    .costs
                    .builtin_costs
                    .un_constr_data([cost_model::data_list_ex_mem(fields)]);

                self.spend_budget(budget)?;

                let list: BumpVec<'_, _> = fields
                    .iter()
                    .map(|d| Constant::data(self.arena, d))
                    .collect_in(self.arena);
                let list = self.arena.alloc(list);

                let constant = Constant::proto_pair(
                    self.arena,
                    Type::integer(self.arena),
                    Type::list(self.arena, Type::data(self.arena)),
                    Constant::integer_from(self.arena, *tag as i128),
                    Constant::proto_list(self.arena, Type::data(self.arena), list),
                );

                let value = Value::con(self.arena, constant);

                Ok(value)
            }
            DefaultFunction::UnMapData => {
                let map = runtime.args[0]
                    .unwrap_constant()?
                    .unwrap_data()?
                    .unwrap_map()?;

                let budget = self
                    .costs
                    .builtin_costs
                    .un_map_data([cost_model::data_map_ex_mem(map)]);

                self.spend_budget(budget)?;

                let list: BumpVec<'_, _> = map
                    .iter()
                    .map(|(k, v)| {
                        Constant::proto_pair(
                            self.arena,
                            Type::data(self.arena),
                            Type::data(self.arena),
                            Constant::data(self.arena, k),
                            Constant::data(self.arena, v),
                        )
                    })
                    .collect_in(self.arena);
                let list = self.arena.alloc(list);

                let constant = Constant::proto_list(
                    self.arena,
                    Type::pair(self.arena, Type::data(self.arena), Type::data(self.arena)),
                    list,
                );

                let value = Value::con(self.arena, constant);

                Ok(value)
            }
            DefaultFunction::UnListData => {
                let list = runtime.args[0]
                    .unwrap_constant()?
                    .unwrap_data()?
                    .unwrap_list()?;

                let budget = self
                    .costs
                    .builtin_costs
                    .un_list_data([cost_model::data_list_ex_mem(list)]);

                self.spend_budget(budget)?;

                let list: BumpVec<'_, _> = list
                    .iter()
                    .map(|d| Constant::data(self.arena, d))
                    .collect_in(self.arena);
                let list = self.arena.alloc(list);

                let constant = Constant::proto_list(self.arena, Type::data(self.arena), list);

                let value = Value::con(self.arena, constant);

                Ok(value)
            }
            DefaultFunction::UnIData => {
                let i = runtime.args[0]
                    .unwrap_constant()?
                    .unwrap_data()?
                    .unwrap_integer()?;

                let budget = self
                    .costs
                    .builtin_costs
                    .un_i_data([cost_model::data_integer_ex_mem(i)]);

                self.spend_budget(budget)?;

                let value = Value::integer(self.arena, i);

                Ok(value)
            }
            DefaultFunction::UnBData => {
                let bs = runtime.args[0]
                    .unwrap_constant()?
                    .unwrap_data()?
                    .unwrap_byte_string()?;

                let budget = self
                    .costs
                    .builtin_costs
                    .un_b_data([cost_model::data_byte_string_ex_mem(bs)]);

                self.spend_budget(budget)?;

                let value = Value::byte_string(self.arena, bs);

                Ok(value)
            }
            DefaultFunction::EqualsData => {
                let d1 = runtime.args[0].unwrap_constant()?.unwrap_data()?;
                let d2 = runtime.args[1].unwrap_constant()?.unwrap_data()?;

                let budget = self
                    .costs
                    .builtin_costs
                    .equals_data([cost_model::data_ex_mem(d1), cost_model::data_ex_mem(d2)]);

                self.spend_budget(budget)?;

                let value = Value::bool(self.arena, d1.eq(d2));

                Ok(value)
            }
            DefaultFunction::SerialiseData => todo!(),
            DefaultFunction::MkPairData => {
                let d1 = runtime.args[0].unwrap_constant()?.unwrap_data()?;
                let d2 = runtime.args[1].unwrap_constant()?.unwrap_data()?;

                let budget = self
                    .costs
                    .builtin_costs
                    .mk_pair_data([cost_model::data_ex_mem(d1), cost_model::data_ex_mem(d2)]);

                self.spend_budget(budget)?;

                let constant = Constant::proto_pair(
                    self.arena,
                    Type::data(self.arena),
                    Type::data(self.arena),
                    Constant::data(self.arena, d1),
                    Constant::data(self.arena, d2),
                );

                let value = Value::con(self.arena, constant);

                Ok(value)
            }
            DefaultFunction::MkNilData => {
                runtime.args[0].unwrap_unit()?;

                let budget = self
                    .costs
                    .builtin_costs
                    .mk_nil_data([cost_model::UNIT_EX_MEM]);

                self.spend_budget(budget)?;

                let list = BumpVec::new_in(self.arena);
                let list = self.arena.alloc(list);

                let constant = Constant::proto_list(self.arena, Type::data(self.arena), list);

                let value = Value::con(self.arena, constant);

                Ok(value)
            }
            DefaultFunction::MkNilPairData => {
                runtime.args[0].unwrap_unit()?;

                let budget = self
                    .costs
                    .builtin_costs
                    .mk_nil_pair_data([cost_model::UNIT_EX_MEM]);

                self.spend_budget(budget)?;

                let list = BumpVec::new_in(self.arena);
                let list = self.arena.alloc(list);

                let constant = Constant::proto_list(
                    self.arena,
                    Type::pair(self.arena, Type::data(self.arena), Type::data(self.arena)),
                    list,
                );

                let value = Value::con(self.arena, constant);

                Ok(value)
            }
            DefaultFunction::Bls12_381_G1_Add => {
                let arg1 = runtime.args[0].unwrap_bls12_381_g1_element()?;
                let arg2 = runtime.args[1].unwrap_bls12_381_g1_element()?;

                let budget = self.costs.builtin_costs.bls12_381_g1_add([
                    cost_model::g1_element_ex_mem(),
                    cost_model::g1_element_ex_mem(),
                ]);

                self.spend_budget(budget)?;

                let out = self.arena.alloc(blst::blst_p1::default());

                unsafe {
                    blst::blst_p1_add_or_double(out as *mut _, arg1 as *const _, arg2 as *const _);
                }

                let constant = Constant::g1(self.arena, out);

                let value = Value::con(self.arena, constant);

                Ok(value)
            }
            DefaultFunction::Bls12_381_G1_Neg => {
                let arg1 = runtime.args[0].unwrap_bls12_381_g1_element()?;

                let budget = self
                    .costs
                    .builtin_costs
                    .bls12_381_g1_neg([cost_model::g1_element_ex_mem()]);

                self.spend_budget(budget)?;

                let out = self.arena.alloc(*arg1);

                unsafe {
                    // second arg was true in the Cardano code
                    blst::blst_p1_cneg(out as *mut _, true);
                }

                let constant = Constant::g1(self.arena, out);

                let value = Value::con(self.arena, constant);

                Ok(value)
            }
            DefaultFunction::Bls12_381_G1_ScalarMul => {
                let arg1 = runtime.args[0].unwrap_integer()?;
                let arg2 = runtime.args[1].unwrap_bls12_381_g1_element()?;

                let budget = self.costs.builtin_costs.bls12_381_g1_scalar_mul([
                    cost_model::integer_ex_mem(arg1),
                    cost_model::g1_element_ex_mem(),
                ]);

                self.spend_budget(budget)?;

                let size_scalar = size_of::<blst::blst_scalar>();

                let arg1 = arg1.mod_floor(&SCALAR_PERIOD);
                let (_, mut arg1) = arg1.to_bytes_be();

                if size_scalar > arg1.len() {
                    let diff = size_scalar - arg1.len();

                    let mut new_vec = vec![0; diff];

                    new_vec.append(&mut arg1);

                    arg1 = new_vec;
                }

                let out = self.arena.alloc(blst::blst_p1::default());
                let scalar = self.arena.alloc(blst::blst_scalar::default());

                unsafe {
                    blst::blst_scalar_from_bendian(scalar as *mut _, arg1.as_ptr() as *const _);

                    blst::blst_p1_mult(
                        out as *mut _,
                        arg2 as *const _,
                        scalar.b.as_ptr() as *const _,
                        size_scalar * 8,
                    );
                }

                let constant = Constant::g1(self.arena, out);

                let value = Value::con(self.arena, constant);

                Ok(value)
            }
            DefaultFunction::Bls12_381_G1_Equal => {
                let arg1 = runtime.args[0].unwrap_bls12_381_g1_element()?;
                let arg2 = runtime.args[1].unwrap_bls12_381_g1_element()?;

                let budget = self.costs.builtin_costs.bls12_381_g1_equal([
                    cost_model::g1_element_ex_mem(),
                    cost_model::g1_element_ex_mem(),
                ]);

                self.spend_budget(budget)?;

                let is_equal = unsafe { blst::blst_p1_is_equal(arg1, arg2) };

                let value = Value::bool(self.arena, is_equal);

                Ok(value)
            }
            DefaultFunction::Bls12_381_G1_Compress => {
                let arg1 = runtime.args[0].unwrap_bls12_381_g1_element()?;

                let budget = self
                    .costs
                    .builtin_costs
                    .bls12_381_g1_compress([cost_model::g1_element_ex_mem()]);

                self.spend_budget(budget)?;

                let out = arg1.compress(self.arena);

                let value = Value::byte_string(self.arena, out);

                Ok(value)
            }
            DefaultFunction::Bls12_381_G1_Uncompress => {
                let arg1 = runtime.args[0].unwrap_byte_string()?;

                let budget = self
                    .costs
                    .builtin_costs
                    .bls12_381_g1_uncompress([cost_model::byte_string_ex_mem(arg1)]);

                self.spend_budget(budget)?;

                let out = blst::blst_p1::uncompress(self.arena, arg1).map_err(MachineError::bls)?;

                let constant = Constant::g1(self.arena, out);

                let value = Value::con(self.arena, constant);

                Ok(value)
            }
            DefaultFunction::Bls12_381_G1_HashToGroup => {
                let arg1 = runtime.args[0].unwrap_byte_string()?;
                let arg2 = runtime.args[1].unwrap_byte_string()?;

                let budget = self.costs.builtin_costs.bls12_381_g1_hash_to_group([
                    cost_model::byte_string_ex_mem(arg1),
                    cost_model::byte_string_ex_mem(arg2),
                ]);

                self.spend_budget(budget)?;

                if arg2.len() > 255 {
                    return Err(MachineError::hash_to_curve_dst_too_big());
                }

                let out = self.arena.alloc(blst::blst_p1::default());
                let aug = [];

                unsafe {
                    blst::blst_hash_to_g1(
                        out as *mut _,
                        arg1.as_ptr(),
                        arg1.len(),
                        arg2.as_ptr(),
                        arg2.len(),
                        aug.as_ptr(),
                        0,
                    );
                };

                let constant = Constant::g1(self.arena, out);

                let value = Value::con(self.arena, constant);

                Ok(value)
            }
            DefaultFunction::Bls12_381_G2_Add => {
                let arg1 = runtime.args[0].unwrap_bls12_381_g2_element()?;
                let arg2 = runtime.args[1].unwrap_bls12_381_g2_element()?;

                let budget = self.costs.builtin_costs.bls12_381_g2_add([
                    cost_model::g2_element_ex_mem(),
                    cost_model::g2_element_ex_mem(),
                ]);

                self.spend_budget(budget)?;

                let out = self.arena.alloc(blst::blst_p2::default());

                unsafe {
                    blst::blst_p2_add_or_double(out as *mut _, arg1 as *const _, arg2 as *const _);
                }

                let constant = Constant::g2(self.arena, out);

                let value = Value::con(self.arena, constant);

                Ok(value)
            }
            DefaultFunction::Bls12_381_G2_Neg => {
                let arg1 = runtime.args[0].unwrap_bls12_381_g2_element()?;

                let budget = self
                    .costs
                    .builtin_costs
                    .bls12_381_g2_neg([cost_model::g2_element_ex_mem()]);

                self.spend_budget(budget)?;

                let out = self.arena.alloc(*arg1);

                unsafe {
                    // second arg was true in the Cardano code
                    blst::blst_p2_cneg(out as *mut _, true);
                }

                let constant = Constant::g2(self.arena, out);

                let value = Value::con(self.arena, constant);

                Ok(value)
            }
            DefaultFunction::Bls12_381_G2_ScalarMul => {
                let arg1 = runtime.args[0].unwrap_integer()?;
                let arg2 = runtime.args[1].unwrap_bls12_381_g2_element()?;

                let budget = self.costs.builtin_costs.bls12_381_g2_scalar_mul([
                    cost_model::integer_ex_mem(arg1),
                    cost_model::g2_element_ex_mem(),
                ]);

                self.spend_budget(budget)?;

                let size_scalar = size_of::<blst::blst_scalar>();

                let computation = arg1 % &*SCALAR_PERIOD;

                let new = self.arena.alloc(computation);

                let mut arg1 = integer_to_bytes(self.arena, new, true);

                if size_scalar > arg1.len() {
                    let diff = size_scalar - arg1.len();

                    let mut new_vec = BumpVec::with_capacity_in(diff, self.arena);

                    unsafe {
                        new_vec.set_len(diff);
                    }

                    new_vec.append(&mut arg1);

                    arg1 = new_vec;
                }

                let out = self.arena.alloc(blst::blst_p2::default());
                let scalar = self.arena.alloc(blst::blst_scalar::default());

                unsafe {
                    blst::blst_scalar_from_bendian(scalar as *mut _, arg1.as_ptr() as *const _);

                    blst::blst_p2_mult(
                        out as *mut _,
                        arg2 as *const _,
                        scalar.b.as_ptr() as *const _,
                        size_scalar * 8,
                    );
                }

                let constant = Constant::g2(self.arena, out);

                let value = Value::con(self.arena, constant);

                Ok(value)
            }
            DefaultFunction::Bls12_381_G2_Equal => {
                let arg1 = runtime.args[0].unwrap_bls12_381_g2_element()?;
                let arg2 = runtime.args[1].unwrap_bls12_381_g2_element()?;

                let budget = self.costs.builtin_costs.bls12_381_g2_equal([
                    cost_model::g2_element_ex_mem(),
                    cost_model::g2_element_ex_mem(),
                ]);

                self.spend_budget(budget)?;

                let is_equal = unsafe { blst::blst_p2_is_equal(arg1, arg2) };

                let value = Value::bool(self.arena, is_equal);

                Ok(value)
            }
            DefaultFunction::Bls12_381_G2_Compress => {
                let arg1 = runtime.args[0].unwrap_bls12_381_g2_element()?;

                let budget = self
                    .costs
                    .builtin_costs
                    .bls12_381_g2_compress([cost_model::g2_element_ex_mem()]);

                self.spend_budget(budget)?;

                let out = arg1.compress(self.arena);

                let value = Value::byte_string(self.arena, out);

                Ok(value)
            }
            DefaultFunction::Bls12_381_G2_Uncompress => {
                let arg1 = runtime.args[0].unwrap_byte_string()?;

                let budget = self
                    .costs
                    .builtin_costs
                    .bls12_381_g2_uncompress([cost_model::byte_string_ex_mem(arg1)]);

                self.spend_budget(budget)?;

                let out = blst::blst_p2::uncompress(self.arena, arg1).map_err(MachineError::bls)?;

                let constant = Constant::g2(self.arena, out);

                let value = Value::con(self.arena, constant);

                Ok(value)
            }
            DefaultFunction::Bls12_381_G2_HashToGroup => {
                let arg1 = runtime.args[0].unwrap_byte_string()?;
                let arg2 = runtime.args[1].unwrap_byte_string()?;

                let budget = self.costs.builtin_costs.bls12_381_g2_hash_to_group([
                    cost_model::byte_string_ex_mem(arg1),
                    cost_model::byte_string_ex_mem(arg2),
                ]);

                self.spend_budget(budget)?;

                if arg2.len() > 255 {
                    return Err(MachineError::hash_to_curve_dst_too_big());
                }

                let out = self.arena.alloc(blst::blst_p2::default());
                let aug = [];

                unsafe {
                    blst::blst_hash_to_g2(
                        out as *mut _,
                        arg1.as_ptr(),
                        arg1.len(),
                        arg2.as_ptr(),
                        arg2.len(),
                        aug.as_ptr(),
                        0,
                    );
                };

                let constant = Constant::g2(self.arena, out);

                let value = Value::con(self.arena, constant);

                Ok(value)
            }
            DefaultFunction::Bls12_381_MillerLoop => {
                let arg1 = runtime.args[0].unwrap_bls12_381_g1_element()?;
                let arg2 = runtime.args[1].unwrap_bls12_381_g2_element()?;

                let budget = self.costs.builtin_costs.bls12_381_miller_loop([
                    cost_model::g1_element_ex_mem(),
                    cost_model::g2_element_ex_mem(),
                ]);

                self.spend_budget(budget)?;

                let out = self.arena.alloc(blst::blst_fp12::default());

                let affine1 = self.arena.alloc(blst::blst_p1_affine::default());
                let affine2 = self.arena.alloc(blst::blst_p2_affine::default());

                unsafe {
                    blst::blst_p1_to_affine(affine1 as *mut _, arg1);
                    blst::blst_p2_to_affine(affine2 as *mut _, arg2);

                    blst::blst_miller_loop(out as *mut _, affine2, affine1);
                }

                let constant = Constant::ml_result(self.arena, out);

                let value = Value::con(self.arena, constant);

                Ok(value)
            }
            DefaultFunction::Bls12_381_MulMlResult => {
                let arg1 = runtime.args[0].unwrap_bls12_381_ml_result()?;
                let arg2 = runtime.args[1].unwrap_bls12_381_ml_result()?;

                let budget = self.costs.builtin_costs.bls12_381_mul_ml_result([
                    cost_model::ml_result_ex_mem(),
                    cost_model::ml_result_ex_mem(),
                ]);

                self.spend_budget(budget)?;

                let out = self.arena.alloc(blst::blst_fp12::default());

                unsafe {
                    blst::blst_fp12_mul(out as *mut _, arg1, arg2);
                }

                let constant = Constant::ml_result(self.arena, out);

                let value = Value::con(self.arena, constant);

                Ok(value)
            }
            DefaultFunction::Bls12_381_FinalVerify => {
                let arg1 = runtime.args[0].unwrap_bls12_381_ml_result()?;
                let arg2 = runtime.args[1].unwrap_bls12_381_ml_result()?;

                let budget = self.costs.builtin_costs.bls12_381_final_verify([
                    cost_model::ml_result_ex_mem(),
                    cost_model::ml_result_ex_mem(),
                ]);

                self.spend_budget(budget)?;

                let verified = unsafe { blst::blst_fp12_finalverify(arg1, arg2) };

                let value = Value::bool(self.arena, verified);

                Ok(value)
            }
            DefaultFunction::IntegerToByteString => {
                let endianness = runtime.args[0].unwrap_bool()?;
                let size = runtime.args[1].unwrap_integer()?;
                let input = runtime.args[2].unwrap_integer()?;

                if size.is_negative() {
                    return Err(MachineError::integer_to_byte_string_negative_size(size));
                }

                if *size > INTEGER_TO_BYTE_STRING_MAXIMUM_OUTPUT_LENGTH.into() {
                    return Err(MachineError::integer_to_byte_string_size_too_big(
                        size,
                        INTEGER_TO_BYTE_STRING_MAXIMUM_OUTPUT_LENGTH,
                    ));
                }

                let arg1: i64 = i64::try_from(size).unwrap();

                let arg1_exmem = if arg1 == 0 { 0 } else { ((arg1 - 1) / 8) + 1 };

                let budget = self.costs.builtin_costs.integer_to_byte_string([
                    cost_model::BOOL_EX_MEM,
                    arg1_exmem,
                    cost_model::integer_ex_mem(input),
                ]);

                self.spend_budget(budget)?;

                // NOTE:
                // We ought to also check for negative size and too large sizes. These checks
                // however happens prior to calling the builtin as part of the costing step. So by
                // the time we reach this builtin call, the size can be assumed to be
                //
                // >= 0 && < INTEGER_TO_BYTE_STRING_MAXIMUM_OUTPUT_LENGTH

                if size.is_zero()
                    && cost_model::integer_log2_x(input)
                        >= 8 * INTEGER_TO_BYTE_STRING_MAXIMUM_OUTPUT_LENGTH
                {
                    let required = cost_model::integer_log2_x(input) / 8 + 1;

                    return Err(MachineError::integer_to_byte_string_size_too_big(
                        constant::integer_from(self.arena, required as i128),
                        INTEGER_TO_BYTE_STRING_MAXIMUM_OUTPUT_LENGTH,
                    ));
                }

                if input.is_negative() {
                    return Err(MachineError::integer_to_byte_string_negative_input(input));
                }

                let size_unwrapped: usize = size.try_into().unwrap();

                if input.is_zero() {
                    let mut new_bytes = BumpVec::with_capacity_in(size_unwrapped, self.arena);

                    unsafe {
                        new_bytes.set_len(size_unwrapped);
                    }

                    new_bytes.fill(0);

                    let new_bytes = self.arena.alloc(new_bytes);

                    let value = Value::byte_string(self.arena, new_bytes);

                    return Ok(value);
                }

                let mut bytes = if endianness {
                    integer_to_bytes(self.arena, input, true)
                } else {
                    integer_to_bytes(self.arena, input, false)
                };

                if !size.is_zero() && bytes.len() > size_unwrapped {
                    return Err(MachineError::integer_to_byte_string_size_too_small(
                        size,
                        bytes.len(),
                    ));
                }

                if size_unwrapped > 0 {
                    let padding_size = size_unwrapped - bytes.len();

                    let mut padding = BumpVec::with_capacity_in(padding_size, self.arena);

                    unsafe {
                        padding.set_len(padding_size);
                    }

                    padding.fill(0);

                    if endianness {
                        padding.append(&mut bytes);

                        bytes = padding;
                    } else {
                        bytes.append(&mut padding);
                    }
                };

                let bytes = self.arena.alloc(bytes);

                let value = Value::byte_string(self.arena, bytes);

                Ok(value)
            }
            DefaultFunction::ByteStringToInteger => {
                let endianness = runtime.args[0].unwrap_bool()?;
                let bytes = runtime.args[1].unwrap_byte_string()?;

                let budget = self.costs.builtin_costs.byte_string_to_integer([
                    cost_model::BOOL_EX_MEM,
                    cost_model::byte_string_ex_mem(bytes),
                ]);

                self.spend_budget(budget)?;

                let number = if endianness {
                    self.arena
                        .alloc(Integer::from_bytes_be(num_bigint::Sign::Plus, bytes))
                } else {
                    self.arena
                        .alloc(Integer::from_bytes_le(num_bigint::Sign::Plus, bytes))
                };

                let value = Value::integer(self.arena, number);

                Ok(value)
            }
            DefaultFunction::AndByteString => {
                let should_pad = runtime.args[0].unwrap_bool()?;
                let left_bytes = runtime.args[1].unwrap_byte_string()?;
                let right_bytes = runtime.args[2].unwrap_byte_string()?;

                let budget = self.costs.builtin_costs.and_byte_string([
                    cost_model::BOOL_EX_MEM,
                    cost_model::byte_string_ex_mem(left_bytes),
                    cost_model::byte_string_ex_mem(right_bytes),
                ]);

                self.spend_budget(budget)?;

                let bytes_result: Vec<u8> = if should_pad {
                    let max_len = left_bytes.len().max(right_bytes.len());
                    (0..max_len)
                        .map(|index| {
                            let left_byte = left_bytes.get(index).copied().unwrap_or(0xFF);
                            let right_byte = right_bytes.get(index).copied().unwrap_or(0xFF);
                            left_byte & right_byte
                        })
                        .collect()
                } else {
                    left_bytes
                        .iter()
                        .zip(right_bytes)
                        .map(|(b1, b2)| b1 & b2)
                        .collect()
                };
                let result = self.arena.alloc(bytes_result);
                let value = Value::byte_string(self.arena, result);
                Ok(value)
            }
            DefaultFunction::OrByteString => {
                let should_pad = runtime.args[0].unwrap_bool()?;
                let left_bytes = runtime.args[1].unwrap_byte_string()?;
                let right_bytes = runtime.args[2].unwrap_byte_string()?;

                let budget = self.costs.builtin_costs.or_byte_string([
                    cost_model::BOOL_EX_MEM,
                    cost_model::byte_string_ex_mem(left_bytes),
                    cost_model::byte_string_ex_mem(right_bytes),
                ]);

                self.spend_budget(budget)?;

                let bytes_result: Vec<u8> = if should_pad {
                    let max_len = left_bytes.len().max(right_bytes.len());
                    (0..max_len)
                        .map(|index| {
                            let left_byte = left_bytes.get(index).copied().unwrap_or(0x00);
                            let right_byte = right_bytes.get(index).copied().unwrap_or(0x00);
                            left_byte | right_byte
                        })
                        .collect()
                } else {
                    left_bytes
                        .iter()
                        .zip(right_bytes)
                        .map(|(b1, b2)| b1 | b2)
                        .collect()
                };

                let result = self.arena.alloc(bytes_result);
                let value = Value::byte_string(self.arena, result);

                Ok(value)
            }
            DefaultFunction::XorByteString => {
                let should_pad = runtime.args[0].unwrap_bool()?;
                let left_bytes = runtime.args[1].unwrap_byte_string()?;
                let right_bytes = runtime.args[2].unwrap_byte_string()?;

                let budget = self.costs.builtin_costs.or_byte_string([
                    cost_model::BOOL_EX_MEM,
                    cost_model::byte_string_ex_mem(left_bytes),
                    cost_model::byte_string_ex_mem(right_bytes),
                ]);

                self.spend_budget(budget)?;

                let bytes_result: Vec<u8> = if should_pad {
                    let max_len = left_bytes.len().max(right_bytes.len());
                    (0..max_len)
                        .map(|index| {
                            let left_byte = left_bytes.get(index).copied().unwrap_or(0x00);
                            let right_byte = right_bytes.get(index).copied().unwrap_or(0x00);
                            left_byte ^ right_byte
                        })
                        .collect()
                } else {
                    left_bytes
                        .iter()
                        .zip(right_bytes)
                        .map(|(b1, b2)| b1 ^ b2)
                        .collect()
                };

                let result = self.arena.alloc(bytes_result);
                let value = Value::byte_string(self.arena, result);

                Ok(value)
            }
            DefaultFunction::ComplementByteString => {
                let bytes = runtime.args[0].unwrap_byte_string()?;

                let budget = self
                    .costs
                    .builtin_costs
                    .complement_byte_string([cost_model::byte_string_ex_mem(bytes)]);
                self.spend_budget(budget)?;

                let result = self
                    .arena
                    .alloc(bytes.iter().map(|b| b ^ 255).collect::<Vec<_>>());

                Ok(Value::byte_string(self.arena, result))
            }
            DefaultFunction::ReadBit => {
                let bytes = runtime.args[0].unwrap_byte_string()?;
                let bit_index = runtime.args[1].unwrap_integer()?;

                let budget = self.costs.builtin_costs.read_bit([
                    cost_model::byte_string_ex_mem(bytes),
                    cost_model::integer_ex_mem(bit_index),
                ]);

                self.spend_budget(budget)?;

                if bytes.is_empty() {
                    return Err(MachineError::empty_byte_array());
                }

                if bit_index < &Integer::ZERO || bit_index >= &Integer::from(bytes.len() * 8) {
                    return Err(MachineError::read_bit_out_of_bounds(
                        bit_index,
                        bytes.len() * 8,
                    ));
                }

                let (byte_index, bit_offset) = bit_index.div_rem(&8.into());
                let bit_offset = usize::try_from(bit_offset).unwrap();

                let flipped_index = bytes.len() - 1 - usize::try_from(byte_index).unwrap();
                let byte = bytes[flipped_index];

                let bit_test = (byte >> bit_offset) & 1 == 1;

                Ok(Value::bool(self.arena, bit_test))
            }
            DefaultFunction::WriteBits => {
                let mut bytes = runtime.args[0].unwrap_byte_string()?.to_vec();
                let indices = runtime.args[1].unwrap_int_list()?;
                let set_bit = runtime.args[2].unwrap_bool()?;

                let budget = self.costs.builtin_costs.write_bits([
                    cost_model::byte_string_ex_mem(bytes.as_slice()),
                    cost_model::proto_list_ex_mem(indices),
                    cost_model::BOOL_EX_MEM,
                ]);

                self.spend_budget(budget)?;

                for index in indices {
                    let Constant::Integer(bit_index) = index else {
                        unreachable!("bit_index must be an integer")
                    };

                    if *bit_index < &Integer::ZERO || *bit_index >= &Integer::from(bytes.len() * 8)
                    {
                        return Err(MachineError::write_bits_out_of_bounds(
                            bit_index,
                            bytes.len() * 8,
                        ));
                    }

                    let (byte_index, bit_offset) = bit_index.div_rem(&8.into());
                    let bit_offset = usize::try_from(bit_offset).unwrap();
                    let flipped_index = bytes.len() - 1 - usize::try_from(byte_index).unwrap();
                    let bit_mask: u8 = 1 << bit_offset;

                    if set_bit {
                        bytes[flipped_index] |= bit_mask;
                    } else {
                        bytes[flipped_index] &= !bit_mask;
                    }
                }

                let result = self.arena.alloc(bytes);
                Ok(Value::byte_string(self.arena, result))
            }
            DefaultFunction::ReplicateByte => {
                let size = runtime.args[0].unwrap_integer()?;
                let byte = runtime.args[1].unwrap_integer()?;

                if size.is_negative() {
                    return Err(MachineError::replicate_byte_negative_size(size));
                }

                if *size > INTEGER_TO_BYTE_STRING_MAXIMUM_OUTPUT_LENGTH.into() {
                    return Err(MachineError::replicate_byte_size_too_big(
                        size,
                        INTEGER_TO_BYTE_STRING_MAXIMUM_OUTPUT_LENGTH,
                    ));
                }

                let arg0: i64 = i64::try_from(size).unwrap();

                let arg0_ex_mem = if arg0 == 0 { 0 } else { ((arg0 - 1) / 8) + 1 };

                let budget = self
                    .costs
                    .builtin_costs
                    .replicate_byte([arg0_ex_mem, cost_model::integer_ex_mem(byte)]);

                self.spend_budget(budget)?;

                if size.is_zero()
                    && cost_model::integer_log2_x(byte)
                        >= 8 * INTEGER_TO_BYTE_STRING_MAXIMUM_OUTPUT_LENGTH
                {
                    let required = cost_model::integer_log2_x(byte) / 8 + 1;

                    return Err(MachineError::replicate_byte_size_too_big(
                        constant::integer_from(self.arena, required as i128),
                        INTEGER_TO_BYTE_STRING_MAXIMUM_OUTPUT_LENGTH,
                    ));
                }

                if byte.is_negative() {
                    return Err(MachineError::replicate_byte_negative_input(byte));
                }

                let size: usize = size.try_into().unwrap();

                let Ok(byte) = u8::try_from(byte) else {
                    return Err(MachineError::outside_byte_bounds(byte));
                };

                let result = if size == 0 {
                    self.arena.alloc(vec![])
                } else {
                    self.arena.alloc([byte].repeat(size))
                };

                Ok(Value::byte_string(self.arena, result))
            }
            DefaultFunction::ShiftByteString => {
                let bytes = runtime.args[0].unwrap_byte_string()?;
                let shift = runtime.args[1].unwrap_integer()?;

                let arg1: i64 = u64::try_from(shift.abs())
                    .unwrap()
                    .try_into()
                    .unwrap_or(i64::MAX);

                let budget = self
                    .costs
                    .builtin_costs
                    .shift_byte_string([cost_model::byte_string_ex_mem(bytes), arg1]);
                self.spend_budget(budget)?;

                let length = bytes.len();
                let result = self.arena.alloc(vec![0; length]);

                if Integer::from(length) * 8 <= shift.abs() {
                    return Ok(Value::byte_string(self.arena, result));
                }

                let is_shift_left = shift >= &Integer::ZERO;
                let byte_shift = usize::try_from(shift.abs() / 8).unwrap();
                let bit_shift = usize::try_from(shift.abs() % 8).unwrap();

                if is_shift_left {
                    if bit_shift == 0 {
                        // If we can shift entire bytes, that's much simpler
                        let copy_len = length - bit_shift;
                        // For example, consider the following byte array [1,0,1,0,1] being shifted 8 bits (1 byte)
                        // Result: [0,1,0,1,0]
                        result[..copy_len].copy_from_slice(&bytes[byte_shift..]);
                    } else {
                        // This case is a bit trickier, so let's walk through an example:
                        // say we are shifting the following byte string by 12 bits:
                        // [AB CD EF 12]
                        // We know we want to skip the first byte, and shift results 4 bits
                        // In order to shift partial bytes, we need to get the "overflow" from the next byte
                        // That is the complement_shift (in this case 4)
                        // i=0:
                        // src_idx = 0 + 1 = 1
                        // result[0] = CD << 4 = D0
                        // result[0] |= EF >> 4 = D0 | 0E = DE
                        // i=1
                        // src_idx = 1 + 1 = 2
                        // result[1] = EF << 4 = F0
                        // reuslt[1] |= 12 >> 4 = F0 | 01 = F1
                        // i=2
                        // src_idx = 2 + 1 = 3
                        // result[2] = 12 << 4 = 20
                        // 3 + 1  < length = false
                        // So our result is:
                        // [DE F1 20 00]
                        let complement_shift = 8 - bit_shift;
                        #[allow(clippy::needless_range_loop)]
                        for i in 0..(length - byte_shift) {
                            let src_idx = i + byte_shift;

                            result[i] = bytes[src_idx] << bit_shift;
                            if src_idx + 1 < length {
                                result[i] |= bytes[src_idx + 1] >> complement_shift;
                            }
                        }
                    }
                } else {
                    // Right shift has the same logic as left shift with the inverse operations
                    if bit_shift == 0 {
                        let copy_len = length - byte_shift;
                        result[byte_shift..].copy_from_slice(&bytes[..copy_len]);
                    } else {
                        // See left shift case for explanation, but invert all operations
                        let complement_shift = 8 - bit_shift;
                        #[allow(clippy::needless_range_loop)]
                        for i in 0..(length - byte_shift) {
                            let dst_idx = i + byte_shift;
                            result[dst_idx] = bytes[i] >> bit_shift;

                            if i > 0 {
                                result[dst_idx] |= bytes[i - 1] << complement_shift;
                            }
                        }
                    }
                }

                Ok(Value::byte_string(self.arena, result))
            }
            DefaultFunction::RotateByteString => {
                let bytes = runtime.args[0].unwrap_byte_string()?;
                let shift = runtime.args[1].unwrap_integer()?;

                let arg1: i64 = u64::try_from(shift.abs())
                    .unwrap()
                    .try_into()
                    .unwrap_or(i64::MAX);

                let budget = self
                    .costs
                    .builtin_costs
                    .rotate_byte_string([cost_model::byte_string_ex_mem(bytes), arg1]);
                self.spend_budget(budget)?;

                let length = bytes.len();
                let result = self.arena.alloc(bytes.to_vec());

                if bytes.is_empty() {
                    return Ok(Value::byte_string(self.arena, result));
                }

                let shift = shift.mod_floor(&(length * 8).into());
                if shift == Integer::ZERO {
                    return Ok(Value::byte_string(self.arena, result));
                }
                let byte_shift = usize::try_from(&shift / 8).unwrap();
                let bit_shift = usize::try_from(shift % 8).unwrap();

                if bit_shift == 0 {
                    // left rotation is the same as shift left
                    // except the overflowed bits are brought to the right
                    let copy_len = length - byte_shift;

                    result[..copy_len].copy_from_slice(&bytes[byte_shift..(copy_len + byte_shift)]);
                    result[copy_len..].copy_from_slice(&bytes[..byte_shift]);
                } else {
                    let complement_shift = 8 - bit_shift;
                    let wraparound_bits = bytes[0] >> complement_shift;
                    #[allow(clippy::needless_range_loop)]
                    for i in 0..(length - byte_shift) {
                        let src_idx = i + byte_shift;

                        result[i] = bytes[src_idx] << bit_shift;

                        if src_idx + 1 < length {
                            result[i] |= bytes[src_idx + 1] >> complement_shift;
                        } else if byte_shift > 0 {
                            result[i] |= bytes[0] >> complement_shift;
                        } else {
                            // In the case we're doing less than a full byte shift
                            // we still need to wrap the bit
                            result[i] |= wraparound_bits;
                        }
                    }

                    for i in 0..byte_shift {
                        let dst_idx = length - byte_shift + i;
                        result[dst_idx] = bytes[i] << bit_shift;

                        if i + 1 < byte_shift {
                            result[dst_idx] |= bytes[i + 1] >> complement_shift;
                        } else {
                            result[dst_idx] |= bytes[byte_shift] >> complement_shift;
                        }
                    }
                }

                Ok(Value::byte_string(self.arena, result))
            }
            DefaultFunction::CountSetBits => {
                let bytes = runtime.args[0].unwrap_byte_string()?;

                let budget = self
                    .costs
                    .builtin_costs
                    .count_set_bits([cost_model::byte_string_ex_mem(bytes)]);
                self.spend_budget(budget)?;

                let weight: Integer = hamming::weight(bytes).into();
                let result = self.arena.alloc(weight);
                Ok(Value::integer(self.arena, result))
            }
            DefaultFunction::FindFirstSetBit => {
                let bytes = runtime.args[0].unwrap_byte_string()?;

                let budget = self
                    .costs
                    .builtin_costs
                    .find_first_set_bit([cost_model::byte_string_ex_mem(bytes)]);
                self.spend_budget(budget)?;

                let first_bit = bytes
                    .iter()
                    .rev()
                    .enumerate()
                    .find_map(|(byte_index, &byte)| {
                        let reversed_byte = byte.reverse_bits();
                        if reversed_byte == 0 {
                            None
                        } else {
                            let bit_index = reversed_byte.leading_zeros() as usize;
                            Some(isize::try_from(bit_index + byte_index * 8).unwrap())
                        }
                    });

                let first_bit: Integer = first_bit.unwrap_or(-1).into();
                let result = self.arena.alloc(first_bit);
                Ok(Value::integer(self.arena, result))
            }
            DefaultFunction::Ripemd_160 => {
                use cryptoxide::{digest::Digest, ripemd160::Ripemd160};
                let input = runtime.args[0].unwrap_byte_string()?;
                let budget = self
                    .costs
                    .builtin_costs
                    .ripemd_160([cost_model::byte_string_ex_mem(input)]);
                self.spend_budget(budget)?;

                let mut hasher = Ripemd160::new();
                hasher.input(input);
                let result = self.arena.alloc(vec![0; hasher.output_bytes()]);
                hasher.result(result);

                Ok(Value::byte_string(self.arena, result))
            }
        }
    }
}

fn integer_to_bytes<'a>(arena: &'a Bump, num: &'a Integer, big_endian: bool) -> BumpVec<'a, u8> {
    let bytes = if big_endian {
        num.magnitude().to_bytes_be()
    } else {
        num.magnitude().to_bytes_le()
    };

    let mut result = BumpVec::with_capacity_in(bytes.len(), arena);
    result.extend_from_slice(&bytes);
    result
}
