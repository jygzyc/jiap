import { runTasksWithConcurrency } from "../src/android/framework-processor.js";

describe("framework processor concurrency", () => {
  it("runs tasks with a bounded level of concurrency", async () => {
    let active = 0;
    let maxActive = 0;

    const tasks = Array.from({ length: 6 }, (_, index) => async () => {
      active += 1;
      maxActive = Math.max(maxActive, active);
      await new Promise((resolve) => setTimeout(resolve, 20));
      active -= 1;
      return index;
    });

    const results = await runTasksWithConcurrency(tasks, 3);

    expect(results).toEqual([0, 1, 2, 3, 4, 5]);
    expect(maxActive).toBe(3);
  });

  it("preserves task result ordering under concurrent execution", async () => {
    const tasks = [
      async () => {
        await new Promise((resolve) => setTimeout(resolve, 30));
        return "slow";
      },
      async () => {
        await new Promise((resolve) => setTimeout(resolve, 5));
        return "fast";
      },
      async () => {
        await new Promise((resolve) => setTimeout(resolve, 10));
        return "mid";
      },
    ];

    await expect(runTasksWithConcurrency(tasks, 3)).resolves.toEqual(["slow", "fast", "mid"]);
  });
});
