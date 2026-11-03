(module
  (import "env" "memory" (memory 1))
  (memory 1)
  (func (export "api_version") (result i32) (i32.const 1))
  (func (export "alloc") (param i32) (result i32) (i32.const 0))
  (func (export "free") (param i32 i32))
  (func (export "call") (param i32 i32) (result i64) (i64.const 0)))
