--------------------------- MODULE SubmissionWorkflow ---------------------------
EXTENDS Naturals, FiniteSets

(*
This module specifies the public submission contract for the 5-bit Shor ECDLP
benchmark repository.

The model follows the ECDSA Fail organization:
  - contestant submissions package only manifest editablePaths;
  - notes and model attribution are required;
  - the trusted evaluator must rank the artifact after RequiredShots;
  - an accepted submission must improve the lower-is-better frontier;
  - sync/reset only restore editable paths from promoted submissions while the
    harness follows the default branch.
*)

CONSTANTS
    Submitters,
    SubmissionIds,
    Models,
    EditableStates,
    HarnessStates,
    MaxScore,
    MaxSubmissionNoteBytes,
    MaxSubmissionArchiveBytes,
    RequiredShots

ASSUME Submitters # {}
ASSUME SubmissionIds # {}
ASSUME Models # {}
ASSUME EditableStates # {}
ASSUME HarnessStates # {}
ASSUME MaxScore \in Nat \ {0}
ASSUME MaxSubmissionNoteBytes \in Nat \ {0}
ASSUME MaxSubmissionArchiveBytes \in Nat \ {0}
ASSUME RequiredShots \in Nat \ {0}

Statuses == {"queued", "accepted", "rejected"}
EmptySubmission == [status |-> "empty"]

Submission ==
    [ submitter : Submitters,
      editable : EditableStates,
      score : 0..MaxScore,
      ranked : BOOLEAN,
      shots : 0..RequiredShots,
      noteBytes : 0..MaxSubmissionNoteBytes,
      archiveBytes : 0..MaxSubmissionArchiveBytes,
      model : Models,
      status : Statuses ]

SubmissionSlot == Submission \cup {EmptySubmission}

VARIABLES
    mainHarness,
    bestEditable,
    bestScore,
    localEditable,
    localHarness,
    submissions

vars == << mainHarness, bestEditable, bestScore, localEditable, localHarness, submissions >>

TypeOK ==
    /\ mainHarness \in HarnessStates
    /\ bestEditable \in EditableStates
    /\ bestScore \in 0..MaxScore
    /\ localEditable \in [Submitters -> EditableStates]
    /\ localHarness \in [Submitters -> HarnessStates]
    /\ submissions \in [SubmissionIds -> SubmissionSlot]

SubmittedIds == {id \in SubmissionIds : submissions[id] # EmptySubmission}

ValidPackage(rec) ==
    /\ rec.noteBytes \in 1..MaxSubmissionNoteBytes
    /\ rec.archiveBytes \in 1..MaxSubmissionArchiveBytes
    /\ rec.model \in Models

TrustedRanked(rec) ==
    /\ rec.ranked
    /\ rec.shots = RequiredShots

ImprovesFrontier(rec) ==
    rec.score < bestScore

Promotable(rec) ==
    /\ rec.status = "queued"
    /\ ValidPackage(rec)
    /\ TrustedRanked(rec)
    /\ ImprovesFrontier(rec)

Init ==
    /\ mainHarness \in HarnessStates
    /\ bestEditable \in EditableStates
    /\ bestScore = MaxScore
    /\ localEditable = [s \in Submitters |-> bestEditable]
    /\ localHarness = [s \in Submitters |-> mainHarness]
    /\ submissions = [id \in SubmissionIds |-> EmptySubmission]

EditLocal(s, e) ==
    /\ s \in Submitters
    /\ e \in EditableStates
    /\ localEditable' = [localEditable EXCEPT ![s] = e]
    /\ UNCHANGED << mainHarness, bestEditable, bestScore, localHarness, submissions >>

Submit(id, rec) ==
    /\ id \in SubmissionIds
    /\ rec \in Submission
    /\ submissions[id] = EmptySubmission
    /\ rec.status = "queued"
    /\ ValidPackage(rec)
    /\ rec.editable = localEditable[rec.submitter]
    /\ submissions' = [submissions EXCEPT ![id] = rec]
    /\ UNCHANGED << mainHarness, bestEditable, bestScore, localEditable, localHarness >>

Promote(id) ==
    LET rec == submissions[id] IN
    /\ id \in SubmittedIds
    /\ Promotable(rec)
    /\ bestEditable' = rec.editable
    /\ bestScore' = rec.score
    /\ submissions' = [submissions EXCEPT ![id] = [rec EXCEPT !.status = "accepted"]]
    /\ UNCHANGED << mainHarness, localEditable, localHarness >>

Reject(id) ==
    LET rec == submissions[id] IN
    /\ id \in SubmittedIds
    /\ rec.status = "queued"
    /\ ~Promotable(rec)
    /\ submissions' = [submissions EXCEPT ![id] = [rec EXCEPT !.status = "rejected"]]
    /\ UNCHANGED << mainHarness, bestEditable, bestScore, localEditable, localHarness >>

Sync(s) ==
    /\ s \in Submitters
    /\ localHarness' = [localHarness EXCEPT ![s] = mainHarness]
    /\ localEditable' = [localEditable EXCEPT ![s] = bestEditable]
    /\ UNCHANGED << mainHarness, bestEditable, bestScore, submissions >>

ResetToAccepted(s, id) ==
    LET rec == submissions[id] IN
    /\ s \in Submitters
    /\ id \in SubmittedIds
    /\ rec.status = "accepted"
    /\ localHarness' = [localHarness EXCEPT ![s] = mainHarness]
    /\ localEditable' = [localEditable EXCEPT ![s] = rec.editable]
    /\ UNCHANGED << mainHarness, bestEditable, bestScore, submissions >>

UpdateHarness(h) ==
    /\ h \in HarnessStates
    /\ mainHarness' = h
    /\ UNCHANGED << bestEditable, bestScore, localEditable, localHarness, submissions >>

Next ==
    \/ \E s \in Submitters, e \in EditableStates : EditLocal(s, e)
    \/ \E id \in SubmissionIds, rec \in Submission : Submit(id, rec)
    \/ \E id \in SubmittedIds : Promote(id)
    \/ \E id \in SubmittedIds : Reject(id)
    \/ \E s \in Submitters : Sync(s)
    \/ \E s \in Submitters, id \in SubmittedIds : ResetToAccepted(s, id)
    \/ \E h \in HarnessStates : UpdateHarness(h)

Spec == Init /\ [][Next]_vars

AcceptedSubmissionsWereTrusted ==
    \A id \in SubmittedIds :
        LET rec == submissions[id] IN
        rec.status = "accepted" => /\ ValidPackage(rec)
                                   /\ TrustedRanked(rec)

QueuedSubmissionsArePackaged ==
    \A id \in SubmittedIds :
        LET rec == submissions[id] IN
        rec.status = "queued" => ValidPackage(rec)

FrontierScoreIsLowerOrEqualToAccepted ==
    \A id \in SubmittedIds :
        LET rec == submissions[id] IN
        rec.status = "accepted" => bestScore <= rec.score

=============================================================================
