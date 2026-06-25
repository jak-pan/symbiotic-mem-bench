// Tiny rune-mode factory for the "async data + loading flag" pair repeated
// across Overview / Questions / Traces / Compare. Only owns the `$state`
// (the fetch `$effect` stays in each component so it can run its own race
// guard against the `id` prop).
export function createAsyncData<T>(initial: T | null = null) {
  let data = $state<T | null>(initial);
  let loading = $state(true);
  return {
    get data() {
      return data;
    },
    get loading() {
      return loading;
    },
    /** Reset for a new fetch (clears data, marks loading). */
    reset() {
      data = null;
      loading = true;
    },
    /** Publish a successful result. */
    set(value: T) {
      data = value;
      loading = false;
    },
  };
}
