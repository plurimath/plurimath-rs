export async function numericKernel(bytes) {
    const { instance } = await WebAssembly.instantiate(bytes, {});
    const native = instance.exports;
    if (native.plurimath_numeric_abi_version() !== 1)
        throw new Error("unsupported numeric kernel ABI");
    const encoder = new TextEncoder(), decoder = new TextDecoder("utf-8", { fatal: true });
    const evaluateJson = (input) => {
        const data = encoder.encode(input), source = native.plurimath_numeric_alloc(data.length);
        let result = 0;
        try {
            new Uint8Array(native.memory.buffer, source, data.length).set(data);
            result = native.plurimath_numeric_evaluate(source, data.length);
            if (!result)
                throw new Error("numeric kernel runtime failure");
            const memory = new Uint8Array(native.memory.buffer);
            let end = result;
            while (end < memory.length && memory[end] !== 0)
                end++;
            if (end === memory.length)
                throw new Error("unterminated kernel result");
            return decoder.decode(memory.subarray(result, end));
        }
        finally {
            if (result)
                native.plurimath_numeric_result_free(result);
            native.plurimath_numeric_dealloc(source, data.length);
        }
    };
    return { evaluateJson, evaluate: request => JSON.parse(evaluateJson(JSON.stringify(request))) };
}
