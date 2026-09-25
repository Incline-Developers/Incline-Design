export default function () {
    return {
        onProgress: ({ current, total }) => {
            // Reserve the final portion for WebAssembly and graphics initialization.
            if (total > 0) window.inclineDesignStartupProgress(90 * current / total);
        },
        onSuccess: () => window.inclineDesignStartupProgress(95),
        onFailure: error => window.inclineDesignStartupError(error?.message || String(error)),
    };
}
