/** Thin WASM ABI-v1 transport; the Rust library owns numeric semantics. */
export interface NumericKernel {
    evaluateJson(input: string): string;
    evaluate(request: unknown): unknown;
}
export declare function numericKernel(bytes: BufferSource): Promise<NumericKernel>;
