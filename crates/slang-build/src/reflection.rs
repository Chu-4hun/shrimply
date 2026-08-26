//! Rust ABI generation from Slang reflection output.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

const CONSTANT_BUFFER_ALIGNMENT: usize = 16;

struct ReflectedEnum<'a> {
    name: &'a str,
    representation: &'a str,
    variants: Vec<(&'a str, i64)>,
}

struct AbiReflection<'a> {
    enums: Vec<ReflectedEnum<'a>>,
    enum_fields: HashMap<&'a str, HashMap<&'a str, &'a str>>,
}

pub fn generate_module(
    module: &str,
    spirv_filename: &str,
    reflection: &Value,
    abi_reflection: &[u8],
) -> String {
    let module = rust_identifier(module);
    assert_eq!(
        spirv_filename,
        format!("{module}.spv"),
        "SPIR-V artifact must be the module filename under OUT_DIR"
    );
    let parameters = reflection
        .get("parameters")
        .and_then(Value::as_array)
        .expect("Slang reflection has no parameters array");
    let entry_points = reflection
        .get("entryPoints")
        .and_then(Value::as_array)
        .expect("Slang reflection has no entryPoints array");

    let abi = parse_abi(abi_reflection);
    let mut output = format!("pub mod {module} {{\n");
    generate_enums(&abi.enums, &mut output);
    let mut generated_structs = HashSet::new();
    for parameter in parameters {
        let ty = required(parameter, "type");
        let element = match required_string(ty, "kind") {
            "constantBuffer" => Some(required(ty, "elementType")),
            "resource"
                if ty.get("baseShape").and_then(Value::as_str) == Some("structuredBuffer") =>
            {
                ty.get("resultType")
                    .filter(|result| required_string(result, "kind") == "struct")
            }
            _ => None,
        };
        if let Some(element) = element {
            let size = ty
                .get("elementVarLayout")
                .and_then(|layout| layout.get("binding"))
                .map(|binding| required_usize(binding, "size"))
                .unwrap_or_else(|| reflected_struct_size(element));
            generate_structs(
                element,
                size,
                &abi.enum_fields,
                &mut generated_structs,
                &mut output,
            );
        }
    }

    generate_descriptors(parameters, &mut output);
    generate_entry_points(entry_points, &mut output);
    output.push_str(&format!(
        "    pub static SPIRV_BYTES: &[u8] = include_bytes!(concat!(env!(\"OUT_DIR\"), \"/{spirv_filename}\"));\n}}\n"
    ));
    output
}

fn parse_abi(reflection: &[u8]) -> AbiReflection<'_> {
    let reflection = std::str::from_utf8(reflection).expect("Slang ABI reflection must be UTF-8");
    let mut enums = Vec::<ReflectedEnum>::new();
    let mut enum_fields = HashMap::new();
    for line in reflection.lines() {
        let mut columns = line.split('\t');
        let kind = columns.next().expect("ABI declaration kind");
        if kind == "enum-field" {
            let structure = rust_identifier(columns.next().expect("struct name"));
            let field = rust_identifier(columns.next().expect("field name"));
            let enumeration = rust_identifier(columns.next().expect("field enum type"));
            assert!(columns.next().is_none(), "malformed Slang enum field");
            assert!(
                enum_fields
                    .entry(structure)
                    .or_insert_with(HashMap::new)
                    .insert(field, enumeration)
                    .is_none(),
                "duplicate reflected enum field `{structure}.{field}`"
            );
            continue;
        }
        assert_eq!(kind, "enum", "unsupported Slang ABI declaration `{kind}`");
        let name = columns.next().expect("enum name");
        let representation = columns.next().expect("enum representation");
        let variant = columns.next().expect("enum variant");
        let value = columns
            .next()
            .expect("enum value")
            .parse()
            .expect("enum value must be an integer");
        assert!(columns.next().is_none(), "malformed Slang enum reflection");
        rust_identifier(name);
        rust_identifier(variant);
        if enums.last().is_none_or(|reflected| reflected.name != name) {
            assert!(
                !enums.iter().any(|reflected| reflected.name == name),
                "Slang enum `{name}` is not contiguous in reflection"
            );
            enums.push(ReflectedEnum {
                name,
                representation,
                variants: Vec::new(),
            });
        }
        let reflected = enums.last_mut().expect("enum was inserted");
        assert_eq!(
            reflected.representation, representation,
            "Slang enum `{name}` changed representation"
        );
        reflected.variants.push((variant, value));
    }

    AbiReflection { enums, enum_fields }
}

fn generate_enums(enums: &[ReflectedEnum<'_>], output: &mut String) {
    for reflected in enums {
        let rust_representation = match reflected.representation {
            "int32" => "i32",
            "uint32" => "u32",
            representation => panic!("unsupported Slang enum representation `{representation}`"),
        };
        let name = reflected.name;
        output.push_str(&format!(
            "    #[repr({rust_representation})]\n    #[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, serde::Deserialize, serde::Serialize)]\n    pub enum {name} {{\n"
        ));
        for (index, (variant, value)) in reflected.variants.iter().enumerate() {
            if index == 0 {
                output.push_str("        #[default]\n");
            }
            output.push_str(&format!("        {variant} = {value},\n"));
        }
        output.push_str("    }\n");
    }
}

fn generate_descriptors(parameters: &[Value], output: &mut String) {
    output.push_str(
        "    #[derive(Clone, Copy, Debug, Eq, PartialEq)]\n    pub enum DescriptorKind {\n        UniformBuffer,\n        SampledImage,\n        Sampler,\n        AccelerationStructure,\n        StorageImage,\n        StorageBuffer,\n    }\n    #[derive(Clone, Copy, Debug, Eq, PartialEq)]\n    pub struct Descriptor {\n        pub binding: u32,\n        pub set: u32,\n        pub kind: DescriptorKind,\n    }\n",
    );
    let mut descriptors = String::from("    pub const DESCRIPTORS: &[Descriptor] = &[\n");
    for parameter in parameters {
        let Some(binding) = parameter.get("binding") else {
            continue;
        };
        if required_string(binding, "kind") != "descriptorTableSlot" {
            continue;
        }
        let name = screaming_snake(required_string(parameter, "name"));
        let index = required_usize(binding, "index");
        let set = binding.get("space").map_or(0, |space| {
            space
                .as_u64()
                .expect("Slang descriptor space must be an integer") as usize
        });
        output.push_str(&format!(
            "    pub const {name}_BINDING: u32 = {index};\n    pub const {name}_DESCRIPTOR_SET: u32 = {set};\n"
        ));
        let ty = required(parameter, "type");
        let kind = match required_string(ty, "kind") {
            "constantBuffer" => "UniformBuffer",
            "samplerState" => "Sampler",
            "resource" => match required_string(ty, "baseShape") {
                "accelerationStructure" => "AccelerationStructure",
                "structuredBuffer" => "StorageBuffer",
                "texture2D" if ty.get("access").and_then(Value::as_str) == Some("readWrite") => {
                    "StorageImage"
                }
                "texture2D" => "SampledImage",
                shape => panic!("unsupported reflected descriptor resource `{shape}`"),
            },
            kind => panic!("unsupported reflected descriptor type `{kind}`"),
        };
        descriptors.push_str(&format!(
            "        Descriptor {{ binding: {index}, set: {set}, kind: DescriptorKind::{kind} }},\n"
        ));
    }
    descriptors.push_str("    ];\n");
    output.push_str(&descriptors);
}

fn generate_entry_points(entry_points: &[Value], output: &mut String) {
    for entry_point in entry_points {
        let name = required_string(entry_point, "name");
        rust_identifier(name);
        output.push_str(&format!(
            "    pub const {}_ENTRY_POINT: &::std::ffi::CStr = c\"{name}\";\n",
            screaming_snake(name)
        ));
    }
}

fn generate_struct(
    layout: &Value,
    size: usize,
    enum_fields: &HashMap<&str, HashMap<&str, &str>>,
) -> String {
    assert_eq!(
        required_string(layout, "kind"),
        "struct",
        "constant-buffer element must be a struct"
    );
    let name = rust_identifier(required_string(layout, "name"));
    let fields = layout
        .get("fields")
        .and_then(Value::as_array)
        .expect("reflected struct has no fields array");
    let mut output = format!(
        "    #[repr(C, align({CONSTANT_BUFFER_ALIGNMENT}))]\n    #[derive(Clone, Copy, Debug, Default, PartialEq, serde::Deserialize, serde::Serialize)]\n    pub struct {name} {{\n"
    );
    let mut end = 0;
    let mut padding = 0;
    let mut assertions = String::new();
    for field in fields {
        let field_name = rust_identifier(required_string(field, "name"));
        let binding = required(field, "binding");
        assert_eq!(
            required_string(binding, "kind"),
            "uniform",
            "constant-buffer field must have a uniform layout"
        );
        let offset = required_usize(binding, "offset");
        let field_size = required_usize(binding, "size");
        assert!(offset >= end, "overlapping reflected fields in {name}");
        if offset > end {
            output.push_str(&format!(
                "        _padding_{padding}: [u8; {}],\n",
                offset - end
            ));
            padding += 1;
        }
        let rust_type = enum_fields
            .get(name)
            .and_then(|fields| fields.get(field_name))
            .map(|name| (*name).to_owned())
            .unwrap_or_else(|| reflected_rust_type(required(field, "type"), field_size));
        output.push_str(&format!("        pub {field_name}: {rust_type},\n"));
        assertions.push_str(&format!(
            "        assert!(::std::mem::offset_of!({name}, {field_name}) == {offset});\n"
        ));
        end = offset + field_size;
    }
    assert!(
        size >= end,
        "reflected size of {name} is smaller than its fields"
    );
    if size > end {
        output.push_str(&format!(
            "        _padding_{padding}: [u8; {}],\n",
            size - end
        ));
    }
    output.push_str(&format!(
        "    }}\n    const _: () = {{\n        assert!(::std::mem::size_of::<{name}>() == {size});\n        assert!(::std::mem::align_of::<{name}>() == {CONSTANT_BUFFER_ALIGNMENT});\n{assertions}    }};\n"
    ));
    output
}

fn generate_structs(
    layout: &Value,
    size: usize,
    enum_fields: &HashMap<&str, HashMap<&str, &str>>,
    generated: &mut HashSet<String>,
    output: &mut String,
) {
    assert_eq!(
        required_string(layout, "kind"),
        "struct",
        "constant-buffer element must be a struct"
    );
    let name = required_string(layout, "name");
    if generated.contains(name) {
        return;
    }
    for field in required(layout, "fields")
        .as_array()
        .expect("reflected struct has no fields array")
    {
        let field_type = required(field, "type");
        match required_string(field_type, "kind") {
            "struct" => generate_structs(
                field_type,
                required_usize(required(field, "binding"), "size"),
                enum_fields,
                generated,
                output,
            ),
            "array" => {
                let element = required(field_type, "elementType");
                if required_string(element, "kind") == "struct" {
                    generate_structs(
                        element,
                        required_usize(field_type, "uniformStride"),
                        enum_fields,
                        generated,
                        output,
                    );
                }
            }
            _ => {}
        }
    }
    assert!(
        generated.insert(name.to_owned()),
        "duplicate reflected struct `{name}`"
    );
    output.push_str(&generate_struct(layout, size, enum_fields));
}

fn reflected_struct_size(layout: &Value) -> usize {
    layout
        .get("fields")
        .and_then(Value::as_array)
        .expect("reflected struct has no fields array")
        .iter()
        .map(|field| {
            let binding = required(field, "binding");
            required_usize(binding, "offset") + required_usize(binding, "size")
        })
        .max()
        .unwrap_or(0)
}

fn reflected_rust_type(ty: &Value, size: usize) -> String {
    match required_string(ty, "kind") {
        "scalar" => {
            let scalar = rust_scalar(required_string(ty, "scalarType"));
            assert_eq!(size, scalar.1, "unexpected reflected scalar size");
            scalar.0.to_owned()
        }
        "vector" => {
            let scalar = reflected_scalar(required(ty, "elementType"));
            let count = required_usize(ty, "elementCount");
            assert_eq!(size, scalar.1 * count, "padded vectors are not supported");
            match (scalar.0, count) {
                ("f32", 3) => "shrimply_render_core::math::Float3".to_owned(),
                ("f32", 4) => "shrimply_render_core::math::Float4".to_owned(),
                _ => format!("[{}; {count}]", scalar.0),
            }
        }
        "matrix" => {
            let scalar = reflected_scalar(required(ty, "elementType"));
            let rows = required_usize(ty, "rowCount");
            let columns = required_usize(ty, "columnCount");
            assert_eq!(
                size,
                scalar.1 * rows * columns,
                "padded matrices are not supported"
            );
            if scalar.0 == "f32" && rows == 4 && columns == 4 {
                "shrimply_render_core::math::Float4x4".to_owned()
            } else {
                format!("[{}; {}]", scalar.0, rows * columns)
            }
        }
        "struct" => rust_identifier(required_string(ty, "name")).to_owned(),
        "array" => {
            let count = required_usize(ty, "elementCount");
            let stride = required_usize(ty, "uniformStride");
            assert_eq!(size, stride * count, "unexpected reflected array size");
            format!(
                "[{}; {count}]",
                reflected_rust_type(required(ty, "elementType"), stride)
            )
        }
        kind => panic!("unsupported reflected Slang field type `{kind}`"),
    }
}

fn reflected_scalar(ty: &Value) -> (&'static str, usize) {
    assert_eq!(
        required_string(ty, "kind"),
        "scalar",
        "vectors and matrices must contain scalars"
    );
    rust_scalar(required_string(ty, "scalarType"))
}

fn rust_scalar(scalar: &str) -> (&'static str, usize) {
    match scalar {
        "float32" => ("f32", std::mem::size_of::<f32>()),
        "int32" => ("i32", std::mem::size_of::<i32>()),
        "uint32" | "bool" => ("u32", std::mem::size_of::<u32>()),
        _ => panic!("unsupported reflected Slang scalar `{scalar}`"),
    }
}

fn required<'a>(value: &'a Value, field: &str) -> &'a Value {
    value
        .get(field)
        .unwrap_or_else(|| panic!("Slang reflection is missing `{field}`"))
}

fn required_string<'a>(value: &'a Value, field: &str) -> &'a str {
    required(value, field)
        .as_str()
        .unwrap_or_else(|| panic!("Slang reflection `{field}` must be a string"))
}

fn required_usize(value: &Value, field: &str) -> usize {
    required(value, field)
        .as_u64()
        .unwrap_or_else(|| panic!("Slang reflection `{field}` must be an integer")) as usize
}

fn rust_identifier(identifier: &str) -> &str {
    assert!(
        !identifier.is_empty()
            && !identifier.starts_with(|character: char| character.is_ascii_digit())
            && identifier
                .chars()
                .all(|character| character == '_' || character.is_ascii_alphanumeric()),
        "`{identifier}` is not a supported Rust identifier"
    );
    identifier
}

fn screaming_snake(identifier: &str) -> String {
    let mut output = String::new();
    let mut previous_lowercase = false;
    for character in identifier.chars() {
        if character.is_ascii_uppercase() && previous_lowercase {
            output.push('_');
        }
        output.push(character.to_ascii_uppercase());
        previous_lowercase = character.is_ascii_lowercase();
    }
    output
}
