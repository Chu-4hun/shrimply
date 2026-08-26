// Shared ABI reflector used by every Slang shader crate.
#include <cstdint>
#include <fstream>
#include <iostream>
#include <set>
#include <string>

#include "slang-com-ptr.h"
#include "slang.h"

static const char* scalar_name(slang::TypeReflection::ScalarType scalar)
{
    switch (scalar)
    {
    case slang::TypeReflection::Int32:
        return "int32";
    case slang::TypeReflection::UInt32:
        return "uint32";
    default:
        return nullptr;
    }
}

static bool reflect_type(
    slang::TypeReflection* type,
    std::ostream& output,
    std::set<std::string>& reflected_structs,
    std::set<std::string>& reflected_enums)
{
    if (!type)
        return true;
    if (type->getKind() == slang::TypeReflection::Kind::ConstantBuffer
        || type->getKind() == slang::TypeReflection::Kind::ParameterBlock)
        return reflect_type(type->getElementType(), output, reflected_structs, reflected_enums);
    if (type->getKind() == slang::TypeReflection::Kind::Struct)
    {
        if (!reflected_structs.insert(type->getName()).second)
            return true;
        for (unsigned int field_index = 0; field_index < type->getFieldCount(); ++field_index)
        {
            auto field = type->getFieldByIndex(field_index);
            auto field_type = field->getType();
            if (field_type->getKind() == slang::TypeReflection::Kind::Enum)
                output << "enum-field\t" << type->getName() << '\t'
                       << field->getName() << '\t' << field_type->getName() << '\n';
            if (!reflect_type(field_type, output, reflected_structs, reflected_enums))
                return false;
        }
        return true;
    }
    if (type->getKind() != slang::TypeReflection::Kind::Enum
        || !reflected_enums.insert(type->getName()).second)
        return true;

    auto scalar = scalar_name(type->getElementType()->getScalarType());
    if (!scalar)
    {
        std::cerr << "unsupported enum representation: " << type->getName() << '\n';
        return false;
    }
    for (unsigned int case_index = 0; case_index < type->getFieldCount(); ++case_index)
    {
        auto enum_case = type->getFieldByIndex(case_index);
        int64_t value = 0;
        if (SLANG_FAILED(enum_case->getDefaultValueInt(&value)))
        {
            std::cerr << "cannot reflect enum case value: " << enum_case->getName() << '\n';
            return false;
        }
        output << "enum\t" << type->getName() << '\t' << scalar << '\t'
               << enum_case->getName() << '\t' << value << '\n';
    }
    return true;
}

int main(int argc, char** argv)
{
    if (argc != 4)
    {
        std::cerr << "usage: slang-reflect <shader-directory> <module> <output>\n";
        return 2;
    }

    Slang::ComPtr<slang::IGlobalSession> global_session;
    if (SLANG_FAILED(slang::createGlobalSession(global_session.writeRef())))
        return 1;

    const std::string module_path = std::string(argv[1]) + "/modules";
    const char* search_paths[] = {argv[1], module_path.c_str()};
    slang::CompilerOptionEntry capabilities[2] = {};
    capabilities[0].name = slang::CompilerOptionName::Capability;
    capabilities[0].value.intValue0 =
        global_session->findCapability("spvGroupNonUniform");
    capabilities[1].name = slang::CompilerOptionName::Capability;
    capabilities[1].value.intValue0 =
        global_session->findCapability("spvGroupNonUniformBallot");
    slang::TargetDesc target = {};
    target.format = SLANG_SPIRV;
    target.profile = global_session->findProfile("spirv_1_5");
    target.compilerOptionEntries = capabilities;
    target.compilerOptionEntryCount = 2;
    slang::SessionDesc description = {};
    description.searchPathCount = 2;
    description.searchPaths = search_paths;
    description.targetCount = 1;
    description.targets = &target;
    Slang::ComPtr<slang::ISession> session;
    if (SLANG_FAILED(global_session->createSession(description, session.writeRef())))
        return 1;

    Slang::ComPtr<slang::IBlob> diagnostics;
    Slang::ComPtr<slang::IModule> module;
    module = session->loadModule(argv[2], diagnostics.writeRef());
    if (diagnostics)
        std::cerr << static_cast<const char*>(diagnostics->getBufferPointer());
    if (!module)
        return 1;

    std::ofstream output(argv[3]);
    if (!output)
        return 1;
    std::set<std::string> reflected_structs;
    std::set<std::string> reflected_enums;
    auto layout = module->getLayout(0, diagnostics.writeRef());
    if (diagnostics)
        std::cerr << static_cast<const char*>(diagnostics->getBufferPointer());
    if (!layout)
        return 1;
    for (unsigned int parameter_index = 0;
         parameter_index < layout->getParameterCount();
         ++parameter_index)
    {
        auto parameter = layout->getParameterByIndex(parameter_index);
        if (!reflect_type(parameter->getType(), output, reflected_structs, reflected_enums))
            return 1;
    }
}
