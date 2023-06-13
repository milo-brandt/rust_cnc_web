import { cncAxios } from './cncAxios'

function simpleAction(url: string, payload?: string) {
  return () => cncAxios.post(url, payload);
}

export const pause = simpleAction('/command/pause');
export const resume = simpleAction('/command/resume');
export const stop = simpleAction('/command/stop');
export const reset = simpleAction('/command/reset');
export const shutdown = simpleAction('/shutdown');
export const home = simpleAction('/debug/send', '$H');
export const unlock = simpleAction('/debug/send', '$X')
function overrideFactory(root: string) {
  return {
    reset: simpleAction(root + "/reset"),
    plus10: simpleAction(root + "/plus10"),
    plus1: simpleAction(root + "/plus1"),
    minus1: simpleAction(root + "/minus1"),
    minus10: simpleAction(root + "/minus10"),
  }
}
export const feedOverride = overrideFactory('/command/override/feed')
export const spindleOverride = overrideFactory('/command/override/spindle')
export const rapidOverride = {
  reset: simpleAction('/command/override/rapid/reset'),
  half: simpleAction('/command/override/rapid/half'),
  quarter: simpleAction('/command/override/rapid/quarter'),
}