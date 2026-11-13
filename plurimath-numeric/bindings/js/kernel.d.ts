/** Thin WASM ABI-v1 transport; the Rust library owns evaluation semantics. */
export interface NumericKernel {
    evaluateJson(input: string): string;
    evaluate(request: unknown): unknown;
}
export interface NumericKernelExports {
    memory: WebAssembly.Memory;
    plurimath_numeric_abi_version(): number;
    plurimath_numeric_alloc(length: number): number;
    plurimath_numeric_dealloc(pointer: number, length: number): void;
    plurimath_numeric_evaluate(pointer: number, length: number): number;
    plurimath_numeric_result_free(pointer: number): void;
}
export declare function numericKernel(bytes: BufferSource): Promise<NumericKernel>;
/** Use exports initialized by the generated WASM host glue. */
export declare function numericKernelFromExports(native: NumericKernelExports): NumericKernel;
