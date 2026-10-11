<script lang="ts" module>
  // Shared tiny components exported as snippets-free helpers.
  export const STATUS_STYLE: Record<string, string> = {
    DRAFT: 'bg-[var(--hover)] text-muted', SUBMITTED: 'bg-info-soft text-info', AUTHORISED: 'bg-warn-soft text-warn', PAID: 'bg-ok-soft text-ok',
    VOIDED: 'bg-bad-soft text-bad', DELETED: 'bg-bad-soft text-bad', SENT: 'bg-info-soft text-info', ACCEPTED: 'bg-ok-soft text-ok', DECLINED: 'bg-bad-soft text-bad',
    INVOICED: 'bg-accent-soft text-accent', BILLED: 'bg-accent-soft text-accent', POSTED: 'bg-ok-soft text-ok', OVERDUE: 'bg-bad-soft text-bad',
    RECONCILED: 'bg-ok-soft text-ok', UNRECONCILED: 'bg-warn-soft text-warn', ACTIVE: 'bg-ok-soft text-ok', ARCHIVED: 'bg-[var(--hover)] text-muted',
    REGISTERED: 'bg-ok-soft text-ok', DISPOSED: 'bg-[var(--hover)] text-muted', FILED: 'bg-ok-soft text-ok', PENDING: 'bg-warn-soft text-warn', APPROVED: 'bg-ok-soft text-ok'
  };
</script>
