use std::collections::BTreeMap;

use crate::parser::{Argument, Constant, Statement};

pub enum DataValue {
    Direct(Vec<u8>),
    Reference(String),
}

impl DataValue {
    /// Returns the size of the value in bytes.
    pub fn size(&self) -> usize {
        match self {
            DataValue::Direct(data) => data.len(),
            DataValue::Reference(_) => 4,
        }
    }
}

pub fn extract_data_objects(statements: &[Statement]) -> BTreeMap<String, Vec<DataValue>> {
    let mut current_label = None;
    let mut objects = BTreeMap::new();
    for s in statements {
        match s {
            Statement::Label(l) => {
                current_label = Some(l.as_str());
            }
            Statement::Directive(dir, args) => match (dir.as_str(), &args[..]) {
                (".type", [Argument::Symbol(name), Argument::Symbol(kind)])
                    if kind.as_str() == "@object" =>
                {
                    assert!(objects.insert(name.clone(), vec![]).is_none());
                }
                (".zero" | ".ascii" | ".asciz" | ".word" | ".byte", args) => {
                    objects
                        .entry(current_label.unwrap().into())
                        .and_modify(|entry| {
                            entry.extend(extract_data_value(dir.as_str(), args));
                        });
                }
                (".size", [Argument::Symbol(name), Argument::Constant(Constant::Number(n))])
                    if Some(name.as_str()) == current_label =>
                {
                    objects
                        .entry(current_label.unwrap().into())
                        .and_modify(|entry| {
                            let size: usize = entry.iter().map(|v| v.size()).sum();
                            assert!(
                                size as i64 == *n,
                                "Invalid size for data object {name}: computed: {size} vs. specified: {n}"
                            );
                        })
                        .or_insert_with(|| {
                            assert!(*n == 0, "Nonzero size for object without elements: {name}");
                            Default::default()
                        });
                }
                _ => {}
            },
            _ => {}
        }
    }
    objects
}

fn extract_data_value(directive: &str, arguments: &[Argument]) -> Vec<DataValue> {
    match (directive, arguments) {
        (
            ".zero",
            [Argument::Constant(Constant::Number(n))]
            // TODO not clear what the second argument is
            | [Argument::Constant(Constant::Number(n)), _],
        ) => {
            vec![DataValue::Direct(vec![0; *n as usize])]
        }
        (".ascii", [Argument::StringLiteral(data)]) => {
            vec![DataValue::Direct(data.clone())]
        }
        (".asciz", [Argument::StringLiteral(data)]) => {
            let mut data = data.clone();
            data.push(0);
            vec![DataValue::Direct(data)]
        }
        (".word", data) => {
            data
                    .iter()
                    .map(|x| {
                        match x {
                            Argument::Constant(Constant::Number(n)) =>{
                                let n = *n as u32;
                                DataValue::Direct(vec![
                                    (n & 0xff) as u8,
                                    (n >> 8 & 0xff) as u8,
                                    (n >> 16 & 0xff) as u8,
                                    (n >> 24 & 0xff) as u8,
                                ])
                            }
                            Argument::Symbol(sym) => {
                                DataValue::Reference(sym.clone())
                            }
                            _ => panic!("Invalid .word directive")
                        }
                    })
                    .collect::<Vec<DataValue>>()
        }
        (".byte", data) => {
            // TODO alignment?
                vec![DataValue::Direct(data
                    .iter()
                    .map(|x| {
                        if let Argument::Constant(Constant::Number(n)) = x {
                            *n as u8
                        } else {
                            panic!("Invalid argument to .byte directive")
                        }
                    })
                    .collect::<Vec<u8>>())]
        }
        _ => panic!()
    }
}
