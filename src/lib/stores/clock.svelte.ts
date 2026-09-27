/** Current time, refreshed every minute so relative timestamps stay current. */
class Clock {
  now = $state(Date.now());

  start(): () => void {
    const timer = setInterval(() => (this.now = Date.now()), 60_000);
    return () => clearInterval(timer);
  }
}

export const clock = new Clock();
