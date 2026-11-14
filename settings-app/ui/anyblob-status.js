export const SAVE_BUSY_STATUS = "Wait for the current save before changing Anyblob.";

export function clearStaleSaveStatus(status) {
  return status === SAVE_BUSY_STATUS ? "" : status;
}
