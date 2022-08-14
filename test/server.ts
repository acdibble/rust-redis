declare global {
  interface MessageMap {
    start: { command: "start" };
    stop: { command: "stop" };
  }

  interface Window {
    onmessage<K extends keyof MessageMap>(
      event: MessageEvent<MessageMap[K]>
    ): void;
    postMessage(message: any): void;
  }
}

let controller: AbortController | undefined;
let child: Deno.Child<{ args: string[]; signal: AbortSignal }> | undefined;

self.onmessage = async (event) => {
  if ("command" in event.data) {
    if (event.data.command === "start") {
      controller = new AbortController();
      child = Deno.spawnChild("cargo", {
        args: ["run", "--release"],
        signal: controller.signal,
      });
      self.postMessage({ command: "started" });
    } else if (event.data.command === "stop") {
      controller?.abort();
      controller = undefined;
      await child?.output();
      child = undefined;
      self.postMessage({ command: "stopped" });
    }
  }
};

export {};
