# LSP Symbol Reporting Proposal

## Overview

Traditional LSP queries return specific, targeted information (e.g., just documentation or just references). For CLI usage, comprehensive reports would be more valuable, combining multiple LSP requests to provide complete symbol context.

## Core Concept

Instead of separate `docs`, `impl`, `refs` commands, introduce a unified `report` command that aggregates relevant information based on symbol type. The report format adapts to the symbol being queried, providing the most useful information for that symbol type.

## Symbol-Specific Report Formats

### Structs & Enums
```
Symbol: MyStruct (struct)
Defined: src/types.rs:15
Documentation: [hover content]

Fields/Variants:
  • field1: String - [field documentation]
  • field2: Option<i32> - [field documentation]

Implementations (3 found):
  src/types.rs:45-67     - impl MyStruct (3 methods)
  src/types.rs:89-105    - impl Display for MyStruct
  src/ext.rs:12-24       - impl Debug for MyStruct

References (8 total):
  src/main.rs:42         - let instance = MyStruct::new()
  src/lib.rs:156         - fn process(data: &MyStruct)
  tests/unit.rs:23       - MyStruct { field1: "test".into(), .. }
```

### Traits
```
Symbol: MyTrait (trait)
Defined: src/traits.rs:12
Documentation: [hover content]

Methods:
  • required_method(&self) -> String
  • provided_method(&self, x: i32) -> bool - [default implementation]

Implementations (5 found):
  src/types.rs:89        - impl MyTrait for MyStruct
  src/other.rs:45        - impl MyTrait for OtherType
  external crates (3)    - [list if discoverable]

Usage in Bounds (12 found):
  src/generic.rs:15      - fn process<T: MyTrait>(item: T)
  src/lib.rs:234         - where T: MyTrait + Send
```

### Functions & Methods
```
Symbol: process_data (function)
Defined: src/processor.rs:45
Signature: fn process_data(input: &[u8], config: &Config) -> Result<Vec<Data>, Error>
Documentation: [hover content]

Called By (6 references):
  src/main.rs:67         - process_data(&buffer, &cfg)?
  src/lib.rs:123         - let result = process_data(data, config)
  tests/integration.rs:89 - process_data(&test_input, &test_config)

Tests Found:
  tests/unit.rs:45       - test_process_data_success()
  tests/unit.rs:78       - test_process_data_invalid_input()
```

### Modules
```
Symbol: parser (module)
Defined: src/parser/mod.rs
Documentation: [module-level docs]

Public API (8 items):
  • struct Parser - Main parsing interface
  • enum ParseError - Error types for parsing failures
  • fn parse_string(s: &str) -> Result<Ast, ParseError>
  • fn validate_syntax(input: &str) -> bool

Internal Structure:
  parser/lexer.rs        - Tokenization logic
  parser/ast.rs          - AST node definitions
  parser/tests.rs        - Module tests

External Usage (15 references):
  src/main.rs:12         - use parser::{Parser, ParseError}
  src/compiler.rs:23     - parser::parse_string(&source)
```

## Implementation Strategy

### LSP Request Sequence
1. **textDocument/hover** - Get documentation and type information
2. **textDocument/definition** - Find definition location
3. **textDocument/references** - Find all usage locations  
4. **textDocument/documentSymbol** - Get structure within defining file
5. **workspace/symbol** - Find related symbols (implementations, tests)

### Symbol Type Detection
Use hover response and definition context to determine symbol type:
- Parse type signature from hover
- Analyze definition location syntax
- Check for trait/impl keywords in surrounding context

### Output Format
- Concise but comprehensive
- Group related information logically
- Show locations with brief context snippets
- Limit lists to most relevant items (with "... and N more" for large sets)

### Performance Considerations
- Cache hover/definition results between commands
- Batch multiple LSP requests when possible
- Implement progressive disclosure (basic info first, details on request)

This approach transforms individual LSP queries into actionable intelligence, providing developers with comprehensive symbol understanding in a single command.