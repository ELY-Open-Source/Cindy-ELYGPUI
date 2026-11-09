// `groupWorkRuns` through a toy adapter whose flags sit on each item; groups list their ids.
const ms = (item) => item?.ms ?? null;

/** The adapter `src/work_runs/tests.rs` mirrors. */
export function adapter(foldAnswerTail) {
  return {
    isUserBoundary: (i) => !!i.user,
    isContinuationBoundary: (i) => !!i.cont,
    isAnswer: (i) => !!i.answer,
    canPrependToAnswer: (i) => !i.commentary,
    isSealedAnswer: (i) => !!i.sealed,
    isCompactBoundary: (i) => !!i.compact,
    isActivity: (i) => !!i.act,
    isArchivable: (i) => !!i.act || !!i.arch,
    startTimestamp: ms,
    endTimestamp: (i) => i.end ?? ms(i),
    boundaryTimestamp: ms,
    userBoundaryEnd: (i, previous) => ms(i) ?? previous,
    createGroup: (run, next, streaming, boundary) =>
      ({ group: run.map((i) => i.id), next: next?.id ?? null, streaming, boundary }),
    createCompletedGroup: (run, next, boundary) =>
      ({ group: run.map((i) => i.id), next: next?.id ?? null, completed: true, boundary }),
    ...(foldAnswerTail === null ? {} : { foldAnswerTail }),
  };
}

export const run = (groupWorkRuns) => (items, streaming, fold) => groupWorkRuns(items, streaming, adapter(fold));
