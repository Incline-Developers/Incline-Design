//! SCIP's parameters (`paramset.c`).
//!
//! A parameter keeps its value either in itself or, as C's `valueptr` does,
//! in a field elsewhere: a [`Target`] reaches that field through the
//! [`Scip`] that owns it, so a plugin reads its own setting as a plain
//! field, and a change through the parameter set lands there.

use std::{collections::HashMap, rc::Rc};

use crate::{
    def,
    retcode::{Retcode, ScipResult},
    scip::Scip,
};

pub type ParamId = usize;

/// `SCIP_DECL_PARAMCHGD`.
pub type ParamChgd = fn(&mut Scip, ParamId) -> ScipResult;

/// Where a parameter keeps its value: C's `valueptr`.
pub struct Target<T> {
    pub get: Box<dyn Fn(&Scip) -> &T>,
    pub get_mut: Box<dyn Fn(&mut Scip) -> &mut T>,
}

/// A [`Target`] on a field of [`crate::set::Set`].
#[macro_export]
macro_rules! set_target {
    ($ty:ty, $field:ident) => {
        Some(std::rc::Rc::new($crate::paramset::Target::<$ty> {
            get: Box::new(|scip: &$crate::scip::Scip| &scip.set.$field),
            get_mut: Box::new(|scip: &mut $crate::scip::Scip| &mut scip.set.$field),
        }))
    };
}

/// A [`Target`] on field `field` of the plugin `id` in the arena `arena`
/// of [`Scip`] (a plugin's own parameter).
#[macro_export]
macro_rules! field_target {
    ($ty:ty, $arena:ident, $id:expr, $field:ident) => {{
        let id = $id;
        Some(std::rc::Rc::new($crate::paramset::Target::<$ty> {
            get: Box::new(move |scip: &$crate::scip::Scip| &scip.$arena[id].$field),
            get_mut: Box::new(move |scip: &mut $crate::scip::Scip| &mut scip.$arena[id].$field),
        }))
    }};
}

/// A [`Target`] on field `field` of the data `data` (of type `dty`) of the
/// plugin `id` in the arena `arena` of [`Scip`] (a parameter stored in a
/// plugin's own data).
#[macro_export]
macro_rules! data_target {
    ($ty:ty, $arena:ident, $id:expr, $data:ident, $dty:ty, $field:ident) => {{
        let id = $id;
        Some(std::rc::Rc::new($crate::paramset::Target::<$ty> {
            get: Box::new(move |scip: &$crate::scip::Scip| &scip.$arena[id].$data.as_ref().unwrap().downcast_ref::<$dty>().unwrap().$field),
            get_mut: Box::new(move |scip: &mut $crate::scip::Scip| &mut scip.$arena[id].$data.as_mut().unwrap().downcast_mut::<$dty>().unwrap().$field),
        }))
    }};
}

/// `SCIP_PARAMSETTING`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamSetting {
    Default = 0,
    Aggressive = 1,
    Fast = 2,
    Off = 3,
}

/// `SCIP_PARAMEMPHASIS`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamEmphasis {
    Default = 0,
    CpSolver = 1,
    EasyCip = 2,
    Feasibility = 3,
    HardLp = 4,
    Optimality = 5,
    Counter = 6,
    PhaseFeas = 7,
    PhaseImprove = 8,
    PhaseProof = 9,
    Numerics = 10,
    Benchmark = 11,
}

pub struct Typed<T> {
    pub target: Option<Rc<Target<T>>>,
    pub curvalue: T,
    pub defaultvalue: T,
}

pub enum ParamData {
    Bool(Typed<bool>),
    Int(Typed<i32>, i32, i32),
    Longint(Typed<i64>, i64, i64),
    Real(Typed<f64>, f64, f64),
    Char(Typed<u8>, Option<String>),
    String(Typed<String>),
}

/// `SCIP_PARAM`.
pub struct Param {
    pub data: ParamData,
    pub name: String,
    pub desc: String,
    pub paramchgd: Option<ParamChgd>,
    pub isadvanced: bool,
    pub isfixed: bool,
}

/// `SCIP_PARAMTYPE`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamType {
    Bool,
    Int,
    Longint,
    Real,
    Char,
    String,
}

impl Param {
    /// `SCIPparamGetType`.
    pub fn get_type(&self) -> ParamType {
        match self.data {
            ParamData::Bool(..) => ParamType::Bool,
            ParamData::Int(..) => ParamType::Int,
            ParamData::Longint(..) => ParamType::Longint,
            ParamData::Real(..) => ParamType::Real,
            ParamData::Char(..) => ParamType::Char,
            ParamData::String(..) => ParamType::String,
        }
    }
}

/// `SCIP_PARAMSET`: the parameters in the order they were added, and their
/// names (C's hash table on the name).
#[derive(Default)]
pub struct ParamSet {
    pub params: Vec<Param>,
    pub hashtable: HashMap<String, ParamId>,
}

macro_rules! typed_value {
    ($scip:expr, $typed:expr) => {
        match &$typed.target {
            Some(target) => (target.get)($scip).clone(),
            None => $typed.curvalue.clone(),
        }
    };
}

/// `SCIPparamGetBool` and the like: the current value, read through the
/// parameter's target when it has one.
pub fn param_get_bool(scip: &Scip, id: ParamId) -> bool {
    match &scip.set.paramset.params[id].data {
        ParamData::Bool(t) => typed_value!(scip, t),
        _ => panic!("parameter <{}> is not a bool", scip.set.paramset.params[id].name),
    }
}

pub fn param_get_int(scip: &Scip, id: ParamId) -> i32 {
    match &scip.set.paramset.params[id].data {
        ParamData::Int(t, ..) => typed_value!(scip, t),
        _ => panic!("parameter <{}> is not an int", scip.set.paramset.params[id].name),
    }
}

pub fn param_get_longint(scip: &Scip, id: ParamId) -> i64 {
    match &scip.set.paramset.params[id].data {
        ParamData::Longint(t, ..) => typed_value!(scip, t),
        _ => panic!("parameter <{}> is not a longint", scip.set.paramset.params[id].name),
    }
}

pub fn param_get_real(scip: &Scip, id: ParamId) -> f64 {
    match &scip.set.paramset.params[id].data {
        ParamData::Real(t, ..) => typed_value!(scip, t),
        _ => panic!("parameter <{}> is not a real", scip.set.paramset.params[id].name),
    }
}

pub fn param_get_char(scip: &Scip, id: ParamId) -> u8 {
    match &scip.set.paramset.params[id].data {
        ParamData::Char(t, _) => typed_value!(scip, t),
        _ => panic!("parameter <{}> is not a char", scip.set.paramset.params[id].name),
    }
}

pub fn param_get_string(scip: &Scip, id: ParamId) -> String {
    match &scip.set.paramset.params[id].data {
        ParamData::String(t) => typed_value!(scip, t),
        _ => panic!("parameter <{}> is not a string", scip.set.paramset.params[id].name),
    }
}

/// `SCIPparamIsDefault`.
pub fn param_is_default(scip: &Scip, id: ParamId) -> bool {
    match &scip.set.paramset.params[id].data {
        ParamData::Bool(t) => typed_value!(scip, t) == t.defaultvalue,
        ParamData::Int(t, ..) => typed_value!(scip, t) == t.defaultvalue,
        ParamData::Longint(t, ..) => typed_value!(scip, t) == t.defaultvalue,
        ParamData::Real(t, ..) => def::eps_z(typed_value!(scip, t) - t.defaultvalue, 1e-16),
        ParamData::Char(t, _) => typed_value!(scip, t) == t.defaultvalue,
        ParamData::String(t) => typed_value!(scip, t) == t.defaultvalue,
    }
}

/// `paramWrite` without comments: `name = value`, with ` fix` after a
/// fixed parameter's value; with `onlychanged`, parameters at their
/// default values are written only if fixed.
fn param_write(scip: &Scip, id: ParamId, out: &mut String, onlychanged: bool) {
    let param = &scip.set.paramset.params[id];
    if onlychanged && param_is_default(scip, id) && !param.isfixed {
        return;
    }
    let value = match &param.data {
        ParamData::Bool(t) => (if typed_value!(scip, t) { "TRUE" } else { "FALSE" }).to_string(),
        ParamData::Int(t, ..) => typed_value!(scip, t).to_string(),
        ParamData::Longint(t, ..) => typed_value!(scip, t).to_string(),
        ParamData::Real(t, ..) => crate::misc::fmt_g_prec(typed_value!(scip, t), 15),
        ParamData::Char(t, _) => (typed_value!(scip, t) as char).to_string(),
        ParamData::String(t) => format!("\"{}\"", typed_value!(scip, t)),
    };
    out.push_str(&format!("{} = {value}", param.name));
    if param.isfixed {
        out.push_str(" fix");
    }
    out.push('\n');
}

/// `SCIPparamsetWrite` without comments, as text.
pub fn paramset_write(scip: &Scip, onlychanged: bool) -> String {
    let mut out = String::new();
    for id in 0..scip.set.paramset.params.len() {
        param_write(scip, id, &mut out, onlychanged);
    }
    out
}

/// `paramTestFixed`.
fn param_test_fixed(param: &Param) -> ScipResult {
    if param.isfixed {
        log::error!("parameter <{}> is fixed and cannot be changed. Unfix it to allow changing the value.", param.name);
        return Err(Retcode::ParameterWrongVal);
    }
    Ok(())
}

/// `paramTestChar`.
fn param_test_char(param: &Param, value: u8) -> ScipResult {
    if matches!(value, b'\x08' | b'\x0c' | b'\n' | b'\r' | b'\x0b') {
        log::error!("Invalid value <{}> for char parameter <{}>.", value, param.name);
        return Err(Retcode::ParameterWrongVal);
    }
    if let ParamData::Char(_, Some(allowed)) = &param.data
        && !allowed.as_bytes().contains(&value)
    {
        log::error!("Invalid value <{}> for char parameter <{}>. Must be in set {{{}}}.", value as char, param.name, allowed);
        return Err(Retcode::ParameterWrongVal);
    }
    Ok(())
}

/// `paramTestString`.
fn param_test_string(param: &Param, value: &str) -> ScipResult {
    for (i, c) in value.bytes().enumerate() {
        if matches!(c, b'\x08' | b'\x0c' | b'\n' | b'\r' | b'\x0b') {
            log::error!("Invalid character <{}> in string parameter <{}> at position {}.", c, param.name, i);
            return Err(Retcode::ParameterWrongVal);
        }
    }
    Ok(())
}

/// The body every `SCIPparamSet<Type>` shares: if the value changes (or on
/// initialisation), check the parameter is not fixed, store the value, and
/// unless initialising call the change method, restoring the old value if
/// it answers `SCIP_PARAMETERWRONGVAL`.
macro_rules! param_set_body {
    ($scip:expr, $id:expr, $variant:ident, $ty:ty, $value:expr, $initialize:expr, $callchgd:expr) => {{
        let scip: &mut Scip = $scip;
        let id: ParamId = $id;
        let value = $value;
        let (target, current) = match &scip.set.paramset.params[id].data {
            ParamData::$variant(t, ..) => (t.target.clone(), typed_value!(&*scip, t)),
            _ => return Err(Retcode::ParameterWrongType),
        };
        if $initialize || current != value {
            param_test_fixed(&scip.set.paramset.params[id])?;
            let oldvalue = current;
            store(scip, id, &target, value);
            if !$initialize
                && $callchgd
                && let Some(paramchgd) = scip.set.paramset.params[id].paramchgd
            {
                match paramchgd(scip, id) {
                    Err(Retcode::ParameterWrongVal) => store(scip, id, &target, oldvalue),
                    other => other?,
                }
            }
        }
        fn store(scip: &mut Scip, id: ParamId, target: &Option<Rc<Target<$ty>>>, value: $ty) {
            match target {
                Some(target) => *(target.get_mut)(scip) = value,
                None => match &mut scip.set.paramset.params[id].data {
                    ParamData::$variant(t, ..) => t.curvalue = value,
                    _ => unreachable!(),
                },
            }
        }
        Ok(())
    }};
}

/// `SCIPparamSetBool`. `callchgd` false is C's `set == NULL`.
pub fn param_set_bool(scip: &mut Scip, id: ParamId, value: bool, initialize: bool, callchgd: bool) -> ScipResult {
    param_set_body!(scip, id, Bool, bool, value, initialize, callchgd)
}

/// `SCIPparamSetInt`.
pub fn param_set_int(scip: &mut Scip, id: ParamId, value: i32, initialize: bool, callchgd: bool) -> ScipResult {
    if let ParamData::Int(_, min, max) = &scip.set.paramset.params[id].data
        && (value < *min || value > *max)
    {
        log::error!(
            "Invalid value <{}> for int parameter <{}>. Must be in range [{},{}].",
            value,
            scip.set.paramset.params[id].name,
            min,
            max
        );
        return Err(Retcode::ParameterWrongVal);
    }
    param_set_body!(scip, id, Int, i32, value, initialize, callchgd)
}

/// `SCIPparamSetLongint`.
pub fn param_set_longint(scip: &mut Scip, id: ParamId, value: i64, initialize: bool, callchgd: bool) -> ScipResult {
    if let ParamData::Longint(_, min, max) = &scip.set.paramset.params[id].data
        && (value < *min || value > *max)
    {
        log::error!(
            "Invalid value <{}> for longint parameter <{}>. Must be in range [{},{}].",
            value,
            scip.set.paramset.params[id].name,
            min,
            max
        );
        return Err(Retcode::ParameterWrongVal);
    }
    param_set_body!(scip, id, Longint, i64, value, initialize, callchgd)
}

/// `SCIPparamSetReal`.
pub fn param_set_real(scip: &mut Scip, id: ParamId, value: f64, initialize: bool, callchgd: bool) -> ScipResult {
    let value = def::max(value, def::REAL_MIN);
    let value = def::min(value, def::REAL_MAX);
    if let ParamData::Real(_, min, max) = &scip.set.paramset.params[id].data
        && (value < *min || value > *max)
    {
        log::error!(
            "Invalid value <{}> for real parameter <{}>. Must be in range [{},{}].",
            value,
            scip.set.paramset.params[id].name,
            min,
            max
        );
        return Err(Retcode::ParameterWrongVal);
    }
    param_set_body!(scip, id, Real, f64, value, initialize, callchgd)
}

/// `SCIPparamSetChar`.
pub fn param_set_char(scip: &mut Scip, id: ParamId, value: u8, initialize: bool, callchgd: bool) -> ScipResult {
    param_test_char(&scip.set.paramset.params[id], value)?;
    param_set_body!(scip, id, Char, u8, value, initialize, callchgd)
}

/// `SCIPparamSetString`: unlike the others, always stores and calls the
/// change method, even when the value is the same.
pub fn param_set_string(scip: &mut Scip, id: ParamId, value: &str, initialize: bool, callchgd: bool) -> ScipResult {
    param_test_string(&scip.set.paramset.params[id], value)?;
    param_test_fixed(&scip.set.paramset.params[id])?;
    let (target, oldvalue) = match &scip.set.paramset.params[id].data {
        ParamData::String(t) => (t.target.clone(), typed_value!(&*scip, t)),
        _ => return Err(Retcode::ParameterWrongType),
    };
    let store = |scip: &mut Scip, value: String| match &target {
        Some(target) => *(target.get_mut)(scip) = value,
        None => match &mut scip.set.paramset.params[id].data {
            ParamData::String(t) => t.curvalue = value,
            _ => unreachable!(),
        },
    };
    store(scip, value.to_owned());
    if !initialize
        && callchgd
        && let Some(paramchgd) = scip.set.paramset.params[id].paramchgd
    {
        match paramchgd(scip, id) {
            Err(Retcode::ParameterWrongVal) => store(scip, oldvalue),
            other => other?,
        }
    }
    Ok(())
}

/// `SCIPparamSetToDefault`.
pub fn param_set_to_default(scip: &mut Scip, id: ParamId) -> ScipResult {
    let param = &scip.set.paramset.params[id];
    if param.isfixed {
        return Ok(());
    }
    match &param.data {
        ParamData::Bool(t) => param_set_bool(scip, id, t.defaultvalue, false, true),
        ParamData::Int(t, ..) => param_set_int(scip, id, t.defaultvalue, false, true),
        ParamData::Longint(t, ..) => param_set_longint(scip, id, t.defaultvalue, false, true),
        ParamData::Real(t, ..) => param_set_real(scip, id, t.defaultvalue, false, true),
        ParamData::Char(t, _) => param_set_char(scip, id, t.defaultvalue, false, true),
        ParamData::String(t) => {
            let default = t.defaultvalue.clone();
            param_set_string(scip, id, &default, false, true)
        }
    }
}

/// `paramsetAdd`.
fn paramset_add(scip: &mut Scip, param: Param) -> ScipResult<ParamId> {
    let paramset = &mut scip.set.paramset;
    if paramset.hashtable.contains_key(&param.name) {
        return Err(Retcode::KeyAlreadyExisting);
    }
    let id = paramset.params.len();
    paramset.hashtable.insert(param.name.clone(), id);
    paramset.params.push(param);
    Ok(id)
}

fn new_param(name: &str, desc: &str, data: ParamData, isadvanced: bool, paramchgd: Option<ParamChgd>) -> Param {
    Param {
        data,
        name: name.to_owned(),
        desc: desc.to_owned(),
        paramchgd,
        isadvanced,
        isfixed: false,
    }
}

/// `SCIPparamsetAddBool`: creates the parameter, sets it to its default
/// (through its target) and adds it.
pub fn paramset_add_bool(
    scip: &mut Scip,
    name: &str,
    desc: &str,
    target: Option<Rc<Target<bool>>>,
    isadvanced: bool,
    defaultvalue: bool,
    paramchgd: Option<ParamChgd>,
) -> ScipResult<ParamId> {
    let typed = Typed {
        target,
        curvalue: defaultvalue,
        defaultvalue,
    };
    let id = paramset_add(scip, new_param(name, desc, ParamData::Bool(typed), isadvanced, paramchgd))?;
    param_set_bool(scip, id, defaultvalue, true, false)?;
    Ok(id)
}

/// `SCIPparamsetAddInt`.
pub fn paramset_add_int(
    scip: &mut Scip,
    name: &str,
    desc: &str,
    target: Option<Rc<Target<i32>>>,
    isadvanced: bool,
    defaultvalue: i32,
    minvalue: i32,
    maxvalue: i32,
    paramchgd: Option<ParamChgd>,
) -> ScipResult<ParamId> {
    let typed = Typed {
        target,
        curvalue: defaultvalue,
        defaultvalue,
    };
    let id = paramset_add(scip, new_param(name, desc, ParamData::Int(typed, minvalue, maxvalue), isadvanced, paramchgd))?;
    param_set_int(scip, id, defaultvalue, true, false)?;
    Ok(id)
}

/// `SCIPparamsetAddLongint`.
pub fn paramset_add_longint(
    scip: &mut Scip,
    name: &str,
    desc: &str,
    target: Option<Rc<Target<i64>>>,
    isadvanced: bool,
    defaultvalue: i64,
    minvalue: i64,
    maxvalue: i64,
    paramchgd: Option<ParamChgd>,
) -> ScipResult<ParamId> {
    let typed = Typed {
        target,
        curvalue: defaultvalue,
        defaultvalue,
    };
    let id = paramset_add(scip, new_param(name, desc, ParamData::Longint(typed, minvalue, maxvalue), isadvanced, paramchgd))?;
    param_set_longint(scip, id, defaultvalue, true, false)?;
    Ok(id)
}

/// `SCIPparamsetAddReal`.
pub fn paramset_add_real(
    scip: &mut Scip,
    name: &str,
    desc: &str,
    target: Option<Rc<Target<f64>>>,
    isadvanced: bool,
    defaultvalue: f64,
    minvalue: f64,
    maxvalue: f64,
    paramchgd: Option<ParamChgd>,
) -> ScipResult<ParamId> {
    let typed = Typed {
        target,
        curvalue: defaultvalue,
        defaultvalue,
    };
    let id = paramset_add(scip, new_param(name, desc, ParamData::Real(typed, minvalue, maxvalue), isadvanced, paramchgd))?;
    param_set_real(scip, id, defaultvalue, true, false)?;
    Ok(id)
}

/// `SCIPparamsetAddChar`.
pub fn paramset_add_char(
    scip: &mut Scip,
    name: &str,
    desc: &str,
    target: Option<Rc<Target<u8>>>,
    isadvanced: bool,
    defaultvalue: u8,
    allowedvalues: Option<&str>,
    paramchgd: Option<ParamChgd>,
) -> ScipResult<ParamId> {
    let typed = Typed {
        target,
        curvalue: defaultvalue,
        defaultvalue,
    };
    let id = paramset_add(scip, new_param(name, desc, ParamData::Char(typed, allowedvalues.map(str::to_owned)), isadvanced, paramchgd))?;
    param_set_char(scip, id, defaultvalue, true, false)?;
    Ok(id)
}

/// `SCIPparamsetAddString`.
pub fn paramset_add_string(
    scip: &mut Scip,
    name: &str,
    desc: &str,
    target: Option<Rc<Target<String>>>,
    isadvanced: bool,
    defaultvalue: &str,
    paramchgd: Option<ParamChgd>,
) -> ScipResult<ParamId> {
    let typed = Typed {
        target,
        curvalue: defaultvalue.to_owned(),
        defaultvalue: defaultvalue.to_owned(),
    };
    let id = paramset_add(scip, new_param(name, desc, ParamData::String(typed), isadvanced, paramchgd))?;
    param_set_string(scip, id, defaultvalue, true, false)?;
    Ok(id)
}

/// `SCIPparamsetGetParam`.
pub fn paramset_get_param(scip: &Scip, name: &str) -> Option<ParamId> {
    scip.set.paramset.hashtable.get(name).copied()
}

fn lookup(scip: &Scip, name: &str, ty: ParamType) -> ScipResult<ParamId> {
    let Some(id) = paramset_get_param(scip, name) else {
        log::error!("parameter <{name}> unknown");
        return Err(Retcode::ParameterUnknown);
    };
    if scip.set.paramset.params[id].get_type() != ty {
        log::error!(
            "wrong parameter type - parameter <{name}> has type {:?} instead of {ty:?}",
            scip.set.paramset.params[id].get_type()
        );
        return Err(Retcode::ParameterWrongType);
    }
    Ok(id)
}

/// `SCIPparamsetIsFixed`.
pub fn paramset_is_fixed(scip: &Scip, name: &str) -> ScipResult<bool> {
    match paramset_get_param(scip, name) {
        Some(id) => Ok(scip.set.paramset.params[id].isfixed),
        None => {
            log::error!("parameter <{name}> unknown");
            Err(Retcode::ParameterUnknown)
        }
    }
}

/// `SCIPparamsetFix`.
pub fn paramset_fix(scip: &mut Scip, name: &str, fixed: bool) -> ScipResult {
    match paramset_get_param(scip, name) {
        Some(id) => {
            scip.set.paramset.params[id].isfixed = fixed;
            Ok(())
        }
        None => {
            log::error!("parameter <{name}> unknown");
            Err(Retcode::ParameterUnknown)
        }
    }
}

/// `SCIPparamsetGetBool`.
pub fn paramset_get_bool(scip: &Scip, name: &str) -> ScipResult<bool> {
    Ok(param_get_bool(scip, lookup(scip, name, ParamType::Bool)?))
}

/// `SCIPparamsetGetInt`.
pub fn paramset_get_int(scip: &Scip, name: &str) -> ScipResult<i32> {
    Ok(param_get_int(scip, lookup(scip, name, ParamType::Int)?))
}

/// `SCIPparamsetGetLongint`.
pub fn paramset_get_longint(scip: &Scip, name: &str) -> ScipResult<i64> {
    Ok(param_get_longint(scip, lookup(scip, name, ParamType::Longint)?))
}

/// `SCIPparamsetGetReal`.
pub fn paramset_get_real(scip: &Scip, name: &str) -> ScipResult<f64> {
    Ok(param_get_real(scip, lookup(scip, name, ParamType::Real)?))
}

/// `SCIPparamsetGetChar`.
pub fn paramset_get_char(scip: &Scip, name: &str) -> ScipResult<u8> {
    Ok(param_get_char(scip, lookup(scip, name, ParamType::Char)?))
}

/// `SCIPparamsetGetString`.
pub fn paramset_get_string(scip: &Scip, name: &str) -> ScipResult<String> {
    Ok(param_get_string(scip, lookup(scip, name, ParamType::String)?))
}

/// `SCIPparamsetSetBool`.
pub fn paramset_set_bool(scip: &mut Scip, name: &str, value: bool) -> ScipResult {
    let id = lookup(scip, name, ParamType::Bool)?;
    param_set_bool(scip, id, value, false, true)
}

/// `SCIPparamsetSetInt`.
pub fn paramset_set_int(scip: &mut Scip, name: &str, value: i32) -> ScipResult {
    let id = lookup(scip, name, ParamType::Int)?;
    param_set_int(scip, id, value, false, true)
}

/// `SCIPparamsetSetLongint`.
pub fn paramset_set_longint(scip: &mut Scip, name: &str, value: i64) -> ScipResult {
    let id = lookup(scip, name, ParamType::Longint)?;
    param_set_longint(scip, id, value, false, true)
}

/// `SCIPparamsetSetReal`.
pub fn paramset_set_real(scip: &mut Scip, name: &str, value: f64) -> ScipResult {
    let id = lookup(scip, name, ParamType::Real)?;
    param_set_real(scip, id, value, false, true)
}

/// `SCIPparamsetSetChar`.
pub fn paramset_set_char(scip: &mut Scip, name: &str, value: u8) -> ScipResult {
    let id = lookup(scip, name, ParamType::Char)?;
    param_set_char(scip, id, value, false, true)
}

/// `SCIPparamsetSetString`.
pub fn paramset_set_string(scip: &mut Scip, name: &str, value: &str) -> ScipResult {
    let id = lookup(scip, name, ParamType::String)?;
    param_set_string(scip, id, value, false, true)
}

/// `SCIPparamsetSetToDefaults`.
pub fn paramset_set_to_defaults(scip: &mut Scip) -> ScipResult {
    for id in 0..scip.set.paramset.params.len() {
        param_set_to_default(scip, id)?;
    }
    Ok(())
}

/// `paramSetBool` and its siblings: the quiet setters of hard-coded
/// settings (emphasis and the like), which skip fixed and unknown
/// parameters.
pub(crate) fn hardcoded_set_bool(scip: &mut Scip, name: &str, value: bool) -> ScipResult {
    if let Some(id) = paramset_get_param(scip, name)
        && !scip.set.paramset.params[id].isfixed
    {
        param_set_bool(scip, id, value, false, true)?;
    }
    Ok(())
}

pub(crate) fn hardcoded_set_int(scip: &mut Scip, name: &str, value: i32) -> ScipResult {
    if let Some(id) = paramset_get_param(scip, name)
        && !scip.set.paramset.params[id].isfixed
    {
        param_set_int(scip, id, value, false, true)?;
    }
    Ok(())
}

pub(crate) fn hardcoded_set_longint(scip: &mut Scip, name: &str, value: i64) -> ScipResult {
    if let Some(id) = paramset_get_param(scip, name)
        && !scip.set.paramset.params[id].isfixed
    {
        param_set_longint(scip, id, value, false, true)?;
    }
    Ok(())
}

pub(crate) fn hardcoded_set_real(scip: &mut Scip, name: &str, value: f64) -> ScipResult {
    if let Some(id) = paramset_get_param(scip, name)
        && !scip.set.paramset.params[id].isfixed
    {
        param_set_real(scip, id, value, false, true)?;
    }
    Ok(())
}

/// `SCIPparamsetSetDefaultBool`: changes the default value of an existing
/// bool parameter.
pub fn paramset_set_default_bool(scip: &mut Scip, name: &str, defaultvalue: bool) -> ScipResult {
    let id = lookup(scip, name, ParamType::Bool)?;
    if let ParamData::Bool(t) = &mut scip.set.paramset.params[id].data {
        t.defaultvalue = defaultvalue;
    }
    Ok(())
}

/// `SCIPparamsetSetDefaultInt`: changes the default value of an existing
/// int parameter.
pub fn paramset_set_default_int(scip: &mut Scip, name: &str, defaultvalue: i32) -> ScipResult {
    let id = lookup(scip, name, ParamType::Int)?;
    if let ParamData::Int(t, ..) = &mut scip.set.paramset.params[id].data {
        t.defaultvalue = defaultvalue;
    }
    Ok(())
}

/// `SCIPparamsetSetToDefault`: resets the parameter of the given name, if
/// it exists and is not fixed.
pub(crate) fn paramset_set_to_default(scip: &mut Scip, name: &str) -> ScipResult {
    if let Some(id) = paramset_get_param(scip, name) {
        param_set_to_default(scip, id)?;
    }
    Ok(())
}

/// The names of the plugins of one kind in `SCIP_SET` array order.
fn plugin_names(scip: &Scip, kind: &str) -> Vec<String> {
    let set = &scip.set;
    match kind {
        "heuristics" => (0..set.nheurs).map(|i| scip.heurs[set.heurs[i]].name.clone()).collect(),
        "separating" => (0..set.nsepas).map(|i| scip.sepas[set.sepas[i]].name.clone()).collect(),
        "presolving" => (0..set.npresols).map(|i| scip.presols[set.presols[i]].name.clone()).collect(),
        "propagating" => (0..set.nprops).map(|i| scip.props[set.props[i]].name.clone()).collect(),
        "constraints" => (0..set.nconshdlrs).map(|i| scip.conshdlrs[set.conshdlrs[i]].name.clone()).collect(),
        _ => unreachable!(),
    }
}

/// `SCIPparamsetSetToSubscipsOff`: disables the heuristics and separators
/// that use sub-SCIPs and the components constraint handler.
pub fn set_subscips_off(scip: &mut Scip, _quiet: bool) -> ScipResult {
    // Disable all heuristics that use auxiliary SCIP instances.
    let heurs: Vec<(String, bool)> = (0..scip.set.nheurs)
        .map(|i| &scip.heurs[scip.set.heurs[i]])
        .map(|h| (h.name.clone(), h.usessubscip))
        .collect();
    let sepas: Vec<(String, bool)> = (0..scip.set.nsepas)
        .map(|i| &scip.sepas[scip.set.sepas[i]])
        .map(|s| (s.name.clone(), s.usessubscip))
        .collect();
    for (kind, plugins) in [("heuristics", heurs), ("separating", sepas)] {
        for (name, usessubscip) in plugins {
            if !usessubscip {
                continue;
            }

            // Get frequency parameter of the plugin.
            let paramname = format!("{kind}/{name}/freq");

            // We have to unfix the parameter if it fixed and not already set
            // to -1.
            if paramset_is_fixed(scip, &paramname)? {
                // If the frequency is already set to -1, we do not have to
                // unfix it, but must not try to set it, either.
                if paramset_get_int(scip, &paramname)? == -1 {
                    continue;
                }

                // Unfix parameter.
                paramset_fix(scip, &paramname, false)?;
            }
            hardcoded_set_int(scip, &paramname, -1)?;
        }
    }

    // Turn off components constraint handler.
    hardcoded_set_int(scip, "constraints/components/maxprerounds", 0)?;
    hardcoded_set_int(scip, "constraints/components/propfreq", -1)?;

    // Marking that the sub-SCIPs have been deactivated.
    scip.set.subscipsoff = true;
    Ok(())
}

/// `paramsetSetHeuristicsDefault`.
fn set_heuristics_default(scip: &mut Scip) -> ScipResult {
    for name in plugin_names(scip, "heuristics") {
        paramset_set_to_default(scip, &format!("heuristics/{name}/freq"))?;
        paramset_set_to_default(scip, &format!("heuristics/{name}/maxlpiterofs"))?;
        paramset_set_to_default(scip, &format!("heuristics/{name}/maxlpiterquot"))?;
    }

    // Set specific parameters for RENS heuristic.
    paramset_set_to_default(scip, "heuristics/rens/nodesofs")?;
    paramset_set_to_default(scip, "heuristics/rens/minfixingrate")?;

    // Set specific parameters for Crossover heuristic.
    paramset_set_to_default(scip, "heuristics/crossover/nwaitingnodes")?;
    paramset_set_to_default(scip, "heuristics/crossover/dontwaitatroot")?;
    paramset_set_to_default(scip, "heuristics/crossover/nodesquot")?;
    paramset_set_to_default(scip, "heuristics/crossover/minfixingrate")?;
    Ok(())
}

/// `paramsetSetHeuristicsFast`.
fn set_heuristics_fast(scip: &mut Scip) -> ScipResult {
    const EXPENSIVEHEURFREQS: [&str; 12] = [
        "heuristics/coefdiving/freq",
        "heuristics/distributiondiving/freq",
        "heuristics/feaspump/freq",
        "heuristics/fracdiving/freq",
        "heuristics/guideddiving/freq",
        "heuristics/linesearchdiving/freq",
        "heuristics/nlpdiving/freq",
        "heuristics/subnlp/freq",
        "heuristics/objpscostdiving/freq",
        "heuristics/pscostdiving/freq",
        "heuristics/rootsoldiving/freq",
        "heuristics/veclendiving/freq",
    ];

    set_heuristics_default(scip)?;

    // Disable all heuristics that use subSCIPs.
    let heurs: Vec<(String, bool)> = (0..scip.set.nheurs)
        .map(|i| &scip.heurs[scip.set.heurs[i]])
        .map(|h| (h.name.clone(), h.usessubscip))
        .collect();
    for (name, usessubscip) in heurs {
        if usessubscip {
            hardcoded_set_int(scip, &format!("heuristics/{name}/freq"), -1)?;
        }
    }

    // Explicitly turn off further expensive heuristics, if included.
    for name in EXPENSIVEHEURFREQS {
        hardcoded_set_int(scip, name, -1)?;
    }
    Ok(())
}

/// `paramsetSetHeuristicsOff`.
fn set_heuristics_off(scip: &mut Scip) -> ScipResult {
    set_heuristics_default(scip)?;
    for name in plugin_names(scip, "heuristics") {
        hardcoded_set_int(scip, &format!("heuristics/{name}/freq"), -1)?;
    }
    Ok(())
}

/// `SCIPsetHeuristics` (`SCIPparamsetSetHeuristics`).
pub fn set_heuristics(scip: &mut Scip, paramsetting: ParamSetting, _quiet: bool) -> ScipResult {
    match paramsetting {
        ParamSetting::Default => set_heuristics_default(scip),
        ParamSetting::Off => set_heuristics_off(scip),
        ParamSetting::Fast => set_heuristics_fast(scip),
        ParamSetting::Aggressive => set_heuristics_aggressive(scip),
    }
}

/// `paramsetSetHeuristicsAggressive`.
fn set_heuristics_aggressive(scip: &mut Scip) -> ScipResult {
    set_heuristics_default(scip)?;

    let heurnames: Vec<String> = (0..scip.set.nheurs).map(|i| scip.heurs[scip.set.heurs[i]].name.clone()).collect();
    for heurname in heurnames {
        // Dualval heuristic should stay disabled.
        if heurname == "dualval" {
            continue;
        }

        // The aggressive Benders' decomposition heuristics should remain
        // disabled.
        if heurname.contains("benders") {
            continue;
        }

        // Get frequency parameter of heuristic.
        let paramname = format!("heuristics/{heurname}/freq");
        if let Some(id) = paramset_get_param(scip, &paramname) {
            let ParamData::Int(t, _, _) = &scip.set.paramset.params[id].data else {
                unreachable!("{paramname} is not an int parameter")
            };
            let deffreq = t.defaultvalue;

            // Change frequency to half of the default value, if it is > 0,
            // otherwise set to 20.
            let newfreq = if deffreq == -1 || deffreq == 0 {
                20
            } else {
                let newfreq = scip.set.ceil(deffreq as f64 / 2.0) as i32;
                if newfreq >= 1 { newfreq } else { 1 }
            };

            hardcoded_set_int(scip, &paramname, newfreq)?;

            // LP iteration limits only get increased for heuristics which are
            // activated by default.
            if deffreq > -1 {
                // Construct (possible) parameter name for LP iteration offset.
                let paramname = format!("heuristics/{heurname}/maxlpiterofs");
                if let Some(id) = paramset_get_param(scip, &paramname)
                    && let ParamData::Int(t, _, _) = &scip.set.paramset.params[id].data
                {
                    // Set LP iteration offset to 1.5 time the current value.
                    let value = (1.5 * t.defaultvalue as f64) as i32;
                    hardcoded_set_int(scip, &paramname, value)?;
                }

                // Construct (possible) parameter name for LP iteration quotient
                // parameter.
                let paramname = format!("heuristics/{heurname}/maxlpiterquot");
                if let Some(id) = paramset_get_param(scip, &paramname)
                    && let ParamData::Real(t, _, _) = &scip.set.paramset.params[id].data
                {
                    // Set LP iteration quotient to 1.5 time the current value.
                    let value = 1.5 * t.defaultvalue;
                    hardcoded_set_real(scip, &paramname, value)?;
                }
            }
        }
    }

    // Set specific parameters for RENS heuristic, if the heuristic is included.
    hardcoded_set_longint(scip, "heuristics/rens/nodesofs", 2000)?;
    hardcoded_set_real(scip, "heuristics/rens/minfixingrate", 0.3)?;

    // Set specific parameters for Crossover heuristic, if the heuristic is
    // included.
    hardcoded_set_longint(scip, "heuristics/crossover/nwaitingnodes", 20)?;
    hardcoded_set_bool(scip, "heuristics/crossover/dontwaitatroot", true)?;
    hardcoded_set_real(scip, "heuristics/crossover/nodesquot", 0.15)?;
    hardcoded_set_real(scip, "heuristics/crossover/minfixingrate", 0.5)?;

    // Set specific parameters for Adaptive Large Neighborhood Search heuristic,
    // if the heuristic is included; activate all neighborhoods explicitly (keep
    // list in alphabetic order).
    let neighborhoodnames = [
        "crossover",
        "dins",
        "localbranching",
        "mutation",
        "proximity",
        "rens",
        "rins",
        "trustregion",
        "zeroobjective",
    ];
    for name in neighborhoodnames {
        hardcoded_set_bool(scip, &format!("heuristics/alns/{name}/active"), true)?;
    }
    hardcoded_set_real(scip, "heuristics/alns/nodesquot", 0.2)?;
    hardcoded_set_longint(scip, "heuristics/alns/nodesofs", 2000)?;

    Ok(())
}

/// `SCIPsetEmphasis` (`SCIPparamsetSetEmphasis`); only the emphasis settings
/// the ported plugins use.
pub fn set_emphasis(scip: &mut Scip, paramemphasis: ParamEmphasis, _quiet: bool) -> ScipResult {
    match paramemphasis {
        ParamEmphasis::Feasibility => {
            // Set heuristics aggressive.
            set_heuristics_aggressive(scip)?;

            // Reduce the amount of separation rounds and disable most expensive
            // separators.
            hardcoded_set_int(scip, "separating/maxrounds", 1)?;
            hardcoded_set_int(scip, "separating/maxroundsroot", 5)?;
            hardcoded_set_int(scip, "separating/aggregation/freq", -1)?;
            hardcoded_set_int(scip, "separating/mcf/freq", -1)?;

            // Set priority for node selection "restartdfs" to be higher as the
            // current used one.
            hardcoded_set_int(scip, "nodeselection/restartdfs/stdpriority", i32::MAX / 4)?;
        }
        _ => unimplemented!("parameter emphasis {paramemphasis:?}"),
    }
    Ok(())
}

/// `paramsetSetPresolvingDefault`.
fn set_presolving_default(scip: &mut Scip) -> ScipResult {
    // Reset each individual presolver.
    for name in plugin_names(scip, "presolving") {
        paramset_set_to_default(scip, &format!("presolving/{name}/maxrounds"))?;
    }

    // Reset presolving for each individual propagator.
    for name in plugin_names(scip, "propagating") {
        paramset_set_to_default(scip, &format!("propagating/{name}/maxprerounds"))?;
    }

    // Reset presolving settings for each individual constraint handler.
    for name in plugin_names(scip, "constraints") {
        paramset_set_to_default(scip, &format!("constraints/{name}/maxprerounds"))?;
        paramset_set_to_default(scip, &format!("constraints/{name}/presolpairwise"))?;
    }

    // Explicitly reset parameters of setppc constraint handler, if the
    // constraint handler is included.
    paramset_set_to_default(scip, "constraints/setppc/cliquelifting")?;

    // Explicitly reset parameters of knapsack constraint handler, if the
    // constraint handler is included.
    paramset_set_to_default(scip, "constraints/knapsack/disaggregation")?;

    // Explicitly reset restart and maxrounds parameters.
    paramset_set_to_default(scip, "presolving/maxrestarts")?;
    paramset_set_to_default(scip, "presolving/restartfac")?;
    paramset_set_to_default(scip, "presolving/restartminred")?;
    paramset_set_to_default(scip, "presolving/maxrounds")?;

    // Explicitly reset probing parameters.
    paramset_set_to_default(scip, "propagating/probing/maxuseless")?;
    paramset_set_to_default(scip, "propagating/probing/maxtotaluseless")?;
    paramset_set_to_default(scip, "propagating/probing/maxprerounds")?;
    Ok(())
}

/// `paramsetSetPresolvingFast`.
fn set_presolving_fast(scip: &mut Scip) -> ScipResult {
    // Reset previous changes on presolving parameters.
    set_presolving_default(scip)?;

    // Turn off pairwise comparison for each constraint handler that has this
    // feature.
    for name in plugin_names(scip, "constraints") {
        let paramname = format!("constraints/{name}/presolpairwise");
        if let Some(id) = paramset_get_param(scip, &paramname)
            && scip.set.paramset.params[id].get_type() == ParamType::Bool
        {
            hardcoded_set_bool(scip, &paramname, false)?;
        }
    }

    // Explicitly turn off restarts.
    hardcoded_set_int(scip, "presolving/maxrestarts", 0)?;

    // Explicitly change parameters of presolver convertinttobin, if
    // included.
    hardcoded_set_int(scip, "presolving/convertinttobin/maxrounds", 0)?;

    // Turn off probing, if included.
    hardcoded_set_int(scip, "propagating/probing/maxprerounds", 0)?;

    // Explicitly disable components constraint handler, if included.
    hardcoded_set_int(scip, "constraints/components/maxprerounds", 0)?;

    // Explicitly disable dominated columns presolver, if included.
    hardcoded_set_int(scip, "presolving/domcol/maxrounds", 0)?;

    // Explicitly disable gate extraction presolver, if included.
    hardcoded_set_int(scip, "presolving/gateextraction/maxrounds", 0)?;

    // Explicitly disable sparsify presolver, if included.
    hardcoded_set_int(scip, "presolving/sparsify/maxrounds", 0)?;

    // Explicitly disable dual sparsify presolver, if included.
    hardcoded_set_int(scip, "presolving/dualsparsify/maxrounds", 0)?;

    // Explicitly disable tworowbnd presolver, if included.
    hardcoded_set_int(scip, "presolving/tworowbnd/maxrounds", 0)?;

    // Explicitly forbid the use of implications in logicor presolving.
    hardcoded_set_bool(scip, "constraints/logicor/implications", false)?;
    Ok(())
}

/// `paramsetSetPresolvingOff`.
fn set_presolving_off(scip: &mut Scip) -> ScipResult {
    // Reset previous changes on presolving parameters.
    set_presolving_default(scip)?;

    // Turn each individual presolver off.
    for name in plugin_names(scip, "presolving") {
        hardcoded_set_int(scip, &format!("presolving/{name}/maxrounds"), 0)?;
    }

    // Turn off presolving for each individual propagator.
    for name in plugin_names(scip, "propagating") {
        hardcoded_set_int(scip, &format!("propagating/{name}/maxprerounds"), 0)?;
    }

    // Turn off presolving for each individual constraint handler.
    for name in plugin_names(scip, "constraints") {
        hardcoded_set_int(scip, &format!("constraints/{name}/maxprerounds"), 0)?;
    }

    // Explicitly turn off restarts.
    hardcoded_set_int(scip, "presolving/maxrestarts", 0)?;

    // Set the maximum number of presolving rounds to zero.
    hardcoded_set_int(scip, "presolving/maxrounds", 0)?;
    Ok(())
}

/// `SCIPsetPresolving` (`SCIPparamsetSetPresolving`).
pub fn set_presolving(scip: &mut Scip, paramsetting: ParamSetting, _quiet: bool) -> ScipResult {
    match paramsetting {
        ParamSetting::Default => set_presolving_default(scip),
        ParamSetting::Off => set_presolving_off(scip),
        ParamSetting::Fast => set_presolving_fast(scip),
        ParamSetting::Aggressive => unimplemented!("paramsetSetPresolvingAggressive"),
    }
}

/// `paramsetSetSeparatingDefault`.
fn set_separating_default(scip: &mut Scip) -> ScipResult {
    // Reset separating parameters of all separators.
    for name in plugin_names(scip, "separating") {
        paramset_set_to_default(scip, &format!("separating/{name}/freq"))?;
        paramset_set_to_default(scip, &format!("separating/{name}/maxroundsroot"))?;
        paramset_set_to_default(scip, &format!("separating/{name}/maxsepacutsroot"))?;
    }

    // Reset each individual constraint handler separation settings.
    for name in plugin_names(scip, "constraints") {
        paramset_set_to_default(scip, &format!("constraints/{name}/sepafreq"))?;
        paramset_set_to_default(scip, &format!("constraints/{name}/maxsepacutsroot"))?;
    }

    // Explicitly reset individual parameters.
    for name in [
        "constraints/linear/separateall",
        "cutselection/hybrid/minorthoroot",
        "separating/maxroundsrootsubrun",
        "separating/maxaddrounds",
        "separating/maxcutsroot",
        "separating/poolfreq",
        "separating/aggregation/maxfailsroot",
        "separating/mcf/maxtestdelta",
        "separating/mcf/trynegscaling",
        "separating/aggregation/maxtriesroot",
        "separating/aggregation/maxaggrsroot",
        "separating/maxbounddist",
        "separating/zerohalf/maxslackroot",
        "separating/zerohalf/maxslack",
        "separating/zerohalf/maxsepacutsroot",
        "separating/zerohalf/maxroundsroot",
    ] {
        paramset_set_to_default(scip, name)?;
    }
    Ok(())
}

/// `paramsetSetSeparatingFast`.
fn set_separating_fast(scip: &mut Scip) -> ScipResult {
    // Reset previous changes on separating parameters.
    set_separating_default(scip)?;

    // Explicitly decrease maxbounddist.
    hardcoded_set_real(scip, "separating/maxbounddist", 0.0)?;

    // Explicitly turn off expensive separators, if included.
    hardcoded_set_int(scip, "constraints/and/sepafreq", 0)?;
    hardcoded_set_int(scip, "separating/aggregation/maxroundsroot", 5)?;
    hardcoded_set_int(scip, "separating/aggregation/maxtriesroot", 100)?;
    hardcoded_set_int(scip, "separating/aggregation/maxaggrsroot", 3)?;
    hardcoded_set_int(scip, "separating/aggregation/maxsepacutsroot", 200)?;
    hardcoded_set_real(scip, "separating/zerohalf/maxslackroot", 0.0)?;
    hardcoded_set_real(scip, "separating/zerohalf/maxslack", 0.0)?;
    hardcoded_set_int(scip, "separating/zerohalf/maxsepacutsroot", 200)?;
    hardcoded_set_int(scip, "separating/zerohalf/maxroundsroot", 5)?;
    hardcoded_set_int(scip, "separating/gomory/maxroundsroot", 20)?;
    hardcoded_set_int(scip, "separating/gomory/maxsepacutsroot", 200)?;
    hardcoded_set_int(scip, "separating/mcf/freq", -1)?;
    Ok(())
}

/// `paramsetSetSeparatingOff`.
fn set_separating_off(scip: &mut Scip) -> ScipResult {
    // Reset previous changes on separating parameters.
    set_separating_default(scip)?;

    // Turn each individual separator off.
    for name in plugin_names(scip, "separating") {
        hardcoded_set_int(scip, &format!("separating/{name}/freq"), -1)?;
    }

    // Turn off separation for each individual constraint handler.
    for name in plugin_names(scip, "constraints") {
        hardcoded_set_int(scip, &format!("constraints/{name}/sepafreq"), -1)?;
    }
    Ok(())
}

/// `SCIPsetSeparating` (`SCIPparamsetSetSeparating`).
pub fn set_separating(scip: &mut Scip, paramsetting: ParamSetting, _quiet: bool) -> ScipResult {
    match paramsetting {
        ParamSetting::Default => set_separating_default(scip),
        ParamSetting::Off => set_separating_off(scip),
        ParamSetting::Fast => set_separating_fast(scip),
        ParamSetting::Aggressive => unimplemented!("paramsetSetSeparatingAggressive"),
    }
}
