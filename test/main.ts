import { assertEquals } from "https://deno.land/std@0.149.0/testing/asserts.ts";
import { connect, Redis } from "https://deno.land/x/redis@v0.26.0/mod.ts";
import { delay } from "https://deno.land/std/async/mod.ts";

const startServer = async () => {
  const worker = new Worker(new URL("./server.ts", import.meta.url).href, {
    type: "module",
  });

  await new Promise<void>((resolve) => {
    const started = (event: MessageEvent) => {
      if (event.data.command === "started") {
        resolve();
        worker.removeEventListener("message", started);
      }
    };

    worker.addEventListener("message", started);
    worker.postMessage({ command: "start" });
  });

  return () =>
    new Promise<void>((resolve) => {
      const stopped = (event: MessageEvent) => {
        if (event.data.command === "stopped") {
          resolve();
          worker.removeEventListener("message", stopped);
          worker.terminate();
        }
      };

      worker.addEventListener("message", stopped);
      worker.postMessage({ command: "stop" });
    });
};

const redisTest = (cb: (client: Redis) => Promise<void>) => async () => {
  const stop = await startServer();
  const client = await connect({ hostname: "127.0.0.1", port: 6379 });
  try {
    await cb(client);
  } finally {
    client.close();
    await stop();
  }
};

Deno.test({
  name: "sets and retrieves key",
  fn: redisTest(async (redis) => {
    assertEquals(await redis.set("foo", "bar"), "OK");
    assertEquals(await redis.get("foo"), "bar");
  }),
});

Deno.test({
  name: "sets and retrieves key with expiration",
  fn: redisTest(async (redis) => {
    assertEquals(await redis.set("foo", "bar", { ex: 1 }), "OK");
    assertEquals(await redis.get("foo"), "bar");
    await delay(1000);
    assertEquals(await redis.get("foo"), undefined);
  }),
});
