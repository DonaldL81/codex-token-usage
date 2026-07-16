<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { getVersion } from "@tauri-apps/api/app";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { onDestroy, onMount, tick } from "svelte";

  type Page = "detail" | "monitor" | "stats";
  type SortDirection = "asc" | "desc";
  type DetailSortKey = "node" | "lastTime" | "startTime" | "totalTokens";
  type MonitorSortKey = "projectSession" | "lastTime" | "totalTokens";

  type UsageFilters = {
    dateFrom: string;
    dateTo: string;
    project: string;
    session: string;
    search: string;
    onlyAnomalies: boolean;
  };

  type LoadDashboardOptions = {
    preserveView: boolean;
    filters: UsageFilters;
    includeDetails?: boolean;
    background?: boolean;
  };

  type ScanState = {
    lastCutoffUtc: string | null;
    lastRunUtc: string | null;
    sessionsRoot: string;
    ledgerTokenEvents: number;
    lastRunNewTokenEvents: number;
    lastRunFilesScanned: number;
    lastRunParseErrors: number;
    includeArchived: boolean;
    error: string | null;
  };

  type AppConfig = {
    sessionsRoot: string;
    includeArchived: boolean;
    refreshIntervalSeconds: number;
    includeMessagesInExport: boolean;
    retentionDays: number;
    updateSource: string;
  };

  type ExportResult = {
    path: string;
    rowCount: number;
  };

  type RebuildResult = {
    backupPath: string | null;
    scanState: ScanState;
  };

  type UpdateInfo = {
    currentVersion: string;
    source: string;
    latestVersion: string | null;
    releaseName: string | null;
    publishedAt: string | null;
    releaseNotes: string | null;
    downloadUrl: string | null;
    releasePageUrl: string | null;
    hasUpdate: boolean;
    message: string;
  };

  type DownloadUpdateResult = {
    path: string;
    fileName: string;
    sizeBytes: number;
  };

  type UpdateDownloadProgress = {
    downloadedBytes: number;
    totalBytes: number | null;
    percent: number | null;
  };

  type UpdateRuntimeInfo = {
    currentExePath: string;
    stableEntryPath: string;
    stableEntryExists: boolean;
    isStableEntry: boolean;
  };

  type InstallStableEntryResult = {
    stableEntryPath: string;
    installed: boolean;
  };

  type InstallDownloadedUpdateResult = {
    updaterScriptPath: string;
    stableEntryPath: string;
  };

  type Metrics = {
    totalTokens: number;
    inputTokens: number;
    cachedInputTokens: number;
    nonCachedInputTokens: number;
    outputTokens: number;
    reasoningOutputTokens: number;
    tokenEventCount: number;
    projectCount: number;
    sessionCount: number;
    turnCount: number;
    userMessageCount: number;
    abnormalCount: number;
    cacheRate: number;
    dailyAverageTokens: number;
    hourlyPeakTokens: number;
  };

  type DetailRow = {
    rowKey: string;
    parentKey: string | null;
    hasChildren: boolean;
    level: number;
    kind: string;
    node: string;
    nodeTooltip: string;
    startTime: string;
    lastTime: string;
    time: string;
    project: string;
    sessionId: string;
    turnId: string;
    event: string;
    inputTokens: number;
    cachedInputTokens: number;
    nonCachedInputTokens: number;
    outputTokens: number;
    reasoningOutputTokens: number;
    totalTokens: number;
    status: string;
    statusReason: string;
  };

  type SummaryRow = {
    rowKey: string;
    parentKey: string | null;
    hasChildren: boolean;
    level: number;
    name: string;
    project: string;
    sessionId: string;
    sessionCount: number;
    messageCount: number;
    inputTokens: number;
    cachedInputTokens: number;
    outputTokens: number;
    reasoningOutputTokens: number;
    totalTokens: number;
    status: string;
    statusReason: string;
  };

  type HourlyBucket = {
    date: string;
    hour: number;
    totalTokens: number;
    status: string;
  };

  type TrendBucket = {
    label: string;
    totalTokens: number;
    status: string;
  };

  type Composition = {
    name: string;
    totalTokens: number;
    ratio: number;
    tone: string;
  };

  type TopSession = {
    sessionId: string;
    sessionName: string;
    project: string;
    projectName: string;
    totalTokens: number;
  };

  type TopProject = {
    project: string;
    projectName: string;
    totalTokens: number;
  };

  type FilterOption = {
    value: string;
    label: string;
    title: string;
    project?: string | null;
  };

  type DetailLevelControl = {
    level: number;
    icon: string;
    label: string;
    english: string;
  };

  type TooltipState = {
    visible: boolean;
    text: string;
    x: number;
    y: number;
    placement: "above" | "below";
  };

  type ViewSnapshot = {
    expandedDetailRows: string[];
    detailExpandLevel: number | null;
    windowScrollX: number;
    windowScrollY: number;
    detailScrollLeft: number;
    detailScrollTop: number;
  };

  type HourlyDay = {
    date: string;
    inRange: boolean;
  };

  type DailyTrendBucket = TrendBucket & {
    inRange: boolean;
    displayLabel: string;
  };

  type MonthlyTrendBucket = TrendBucket & {
    inRange: boolean;
    displayLabel: string;
  };

  type CalendarDay = {
    value: string;
    label: number;
    inCurrentMonth: boolean;
    isToday: boolean;
    isStart: boolean;
    isEnd: boolean;
    inRange: boolean;
  };

  type CalendarMonth = {
    label: string;
    days: CalendarDay[];
  };

  type DashboardData = {
    generatedAt: string;
    dataDir: string;
    databasePath: string;
    sessionsRoot: string;
    config: AppConfig;
    scanState: ScanState;
    metrics: Metrics;
    detailRows: DetailRow[];
    summaryRows: SummaryRow[];
    dailyBuckets: TrendBucket[];
    monthlyBuckets: TrendBucket[];
    hourlyBuckets: HourlyBucket[];
    composition: Composition[];
    topSessions: TopSession[];
    topProjects: TopProject[];
    projectOptions: FilterOption[];
    sessionOptions: FilterOption[];
    detailsLoaded: boolean;
    performanceTimings: {
      queryMs: number;
      assemblyMs: number;
      totalMs: number;
    };
  };

  function todayLocalDate(): string {
    const today = new Date();
    const month = `${today.getMonth() + 1}`.padStart(2, "0");
    const day = `${today.getDate()}`.padStart(2, "0");
    return `${today.getFullYear()}-${month}-${day}`;
  }

  function defaultFilters(): UsageFilters {
    const today = parseDateValue(todayLocalDate()) ?? new Date();
    return {
      dateFrom: formatDateValue(addDays(today, -13)),
      dateTo: formatDateValue(today),
      project: "",
      session: "",
      search: "",
      onlyAnomalies: false
    };
  }

  let activePage: Page = "stats";
  let filters: UsageFilters = defaultFilters();
  let appliedFilters: UsageFilters = { ...filters };
  let draftDateFrom = filters.dateFrom;
  let draftDateTo = filters.dateTo;
  let settings: AppConfig = {
    sessionsRoot: "",
    includeArchived: true,
    refreshIntervalSeconds: 0,
    includeMessagesInExport: false,
    retentionDays: 0,
    updateSource: "DonaldL81/codex-token-usage"
  };
  let data: DashboardData | null = null;
  let loading = false;
  let syncing = false;
  let refreshPromise: Promise<void> | null = null;
  let dashboardRequestSequence = 0;
  let activeDashboardRequest: { id: number; includeDetails: boolean } | null = null;
  let syncStatusMessage = "";
  let error = "";
  let showDateRangePicker = false;
  let calendarBaseMonth = startOfMonth(parseDateValue(defaultFilters().dateFrom) ?? new Date());
  let rangeAnchor: string | null = null;
  let settingsMessage = "";
  let updateInfo: UpdateInfo | null = null;
  let appVersion = "";
  let updateRuntimeInfo: UpdateRuntimeInfo | null = null;
  let updateButtonStatus: "idle" | "checking" | "latest" | "ready" | "downloading" | "installing" | "failed" =
    "idle";
  let updateResultMessage = "";
  let updateDownloadProgress: UpdateDownloadProgress | null = null;
  let editingSessionsRoot = false;
  let draftSessionsRoot = settings.sessionsRoot;
  let editingRefreshInterval = false;
  let draftRefreshIntervalSeconds = settings.refreshIntervalSeconds;
  let autoUpdateChecked = false;
  let exportMessage = "";
  let refreshTimer: ReturnType<typeof setInterval> | null = null;
  let trayUnlisten: UnlistenFn | null = null;
  let trayCheckUpdateUnlisten: UnlistenFn | null = null;
  let updateProgressUnlisten: UnlistenFn | null = null;
  let startupUpdateTimer: ReturnType<typeof setTimeout> | null = null;
  let stableEntryEnsured = false;
  let expandedDetailRows = new Set<string>();
  let detailExpandLevel: number | null = 0;
  let showOnlyAnomalies = false;
  let detailSort: { key: DetailSortKey; direction: SortDirection } = { key: "lastTime", direction: "desc" };
  let monitorSort: { key: MonitorSortKey; direction: SortDirection } = { key: "lastTime", direction: "desc" };
  let updatedDetailRows = new Set<string>();
  let monitorUserInputKeys = new Set<string>();
  let detailTableWrap: HTMLDivElement | null = null;
  let tooltip: TooltipState = { visible: false, text: "", x: 0, y: 0, placement: "below" };
  let tooltipHideTimer: ReturnType<typeof setTimeout> | null = null;

  const detailLevelControls: DetailLevelControl[] = [
    { level: 0, icon: "P", label: "项目", english: "Project" },
    { level: 1, icon: "S", label: "会话", english: "Session" },
    { level: 2, icon: "I", label: "用户输入", english: "User Input" },
    { level: 3, icon: "T", label: "轮次", english: "Turn" },
    { level: 4, icon: "#", label: "Token记录", english: "TokenCount" }
  ];
  const metricSkeletonLabels = [
    "当前筛选总 Token",
    "输入 Token",
    "缓存输入",
    "非缓存输入",
    "输出 Token",
    "推理输出",
    "TokenCount",
    "超高行"
  ];
  const DEFAULT_UPDATE_SOURCE = "DonaldL81/codex-token-usage";
  const DASHBOARD_CACHE_KEY = "codex-token-usage-dashboard-cache";
  const monitorStartedAt = new Date();

  $: metrics = data?.metrics;
  $: initialLoading = loading && !data;
  $: filteredSessionOptions = filterSessionOptions(data?.sessionOptions ?? [], filters.project);
  $: detailRows = showOnlyAnomalies
    ? filterDetailRowsByStatus(data?.detailRows ?? [], "正常")
    : data?.detailRows ?? [];
  $: detailRowsByKey = new Map((data?.detailRows ?? []).map((row) => [row.rowKey, row]));
  $: sortedDetailRows = sortDetailTreeRows(detailRows, detailSort);
  $: visibleDetailRows = visibleTreeRows(sortedDetailRows, expandedDetailRows);
  $: monitorRows = sortMonitorRows(
    buildMonitorRows(data?.detailRows ?? [], monitorUserInputKeys, monitorStartedAt),
    monitorSort
  );
  $: monthlyTrendBuckets = buildMonthlyTrendBuckets(appliedFilters, data?.monthlyBuckets ?? []);
  $: dailyTrendBuckets = buildDailyTrendBuckets(appliedFilters, data?.dailyBuckets ?? []);
  $: hourlyDays = buildHourlyDays(appliedFilters);
  $: hourlyMap = buildHourlyMap(data?.hourlyBuckets ?? []);
  $: hourlyPeak = Math.max(...(data?.hourlyBuckets ?? []).map((bucket) => bucket.totalTokens), 0);
  $: dailyPeak = Math.max(...dailyTrendBuckets.map((bucket) => bucket.totalTokens), 0);
  $: monthlyPeak = Math.max(...monthlyTrendBuckets.filter((bucket) => bucket.inRange).map((bucket) => bucket.totalTokens), 0);
  $: topProjectPeak = Math.max(...(data?.topProjects ?? []).map((project) => project.totalTokens), 0);
  $: topSessionPeak = Math.max(...(data?.topSessions ?? []).map((session) => session.totalTokens), 0);
  $: calendarMonths = [
    buildCalendarMonth(calendarBaseMonth, draftDateFrom, draftDateTo),
    buildCalendarMonth(addMonths(calendarBaseMonth, 1), draftDateFrom, draftDateTo)
  ];

  function normalizeConfig(config: AppConfig): AppConfig {
    return {
      ...config,
      includeArchived: true,
      includeMessagesInExport: false,
      updateSource: DEFAULT_UPDATE_SOURCE
    };
  }

  function syncSettingsDrafts(nextSettings = settings) {
    if (!editingSessionsRoot) {
      draftSessionsRoot = nextSettings.sessionsRoot;
    }
    if (!editingRefreshInterval) {
      draftRefreshIntervalSeconds = nextSettings.refreshIntervalSeconds;
    }
  }

  onMount(() => {
    void getVersion()
      .then((version) => {
        appVersion = version;
      })
      .catch(() => {
        appVersion = "";
      });
    restoreUiState();
    void bootstrapUsage();
    startupUpdateTimer = setTimeout(() => {
      startupUpdateTimer = null;
      maybeAutoCheckUpdate();
    }, 1200);
    listen<UpdateDownloadProgress>("update-download-progress", (event) => {
      updateDownloadProgress = event.payload;
    })
      .then((unlisten) => {
        updateProgressUnlisten = unlisten;
      })
      .catch(() => {
        updateProgressUnlisten = null;
      });
    listen("tray-refresh", () => refreshUsage())
      .then((unlisten) => {
        trayUnlisten = unlisten;
      })
      .catch(() => {
        trayUnlisten = null;
      });
    listen("tray-check-update", () => checkUpdate())
      .then((unlisten) => {
        trayCheckUpdateUnlisten = unlisten;
      })
      .catch(() => {
        trayCheckUpdateUnlisten = null;
      });
  });

  onDestroy(() => {
    clearRefreshTimer();
    if (trayUnlisten) {
      trayUnlisten();
    }
    if (trayCheckUpdateUnlisten) {
      trayCheckUpdateUnlisten();
    }
    if (updateProgressUnlisten) {
      updateProgressUnlisten();
    }
    if (startupUpdateTimer) {
      clearTimeout(startupUpdateTimer);
    }
    cancelTooltipHide();
  });

  async function bootstrapUsage() {
    const cached = restoreDashboardCache();
    if (cached) {
      data = cached;
      syncSettingsDrafts(normalizeConfig(cached.config));
      settings = normalizeConfig(cached.config);
      syncStatusMessage = "正在后台同步本机日志...";
      void refreshUsage({ background: true });
      return;
    }
    await refreshUsage();
  }

  async function refreshUsage(options: { background?: boolean } = {}) {
    if (refreshPromise) {
      return refreshPromise;
    }
    normalizeDateRange();
    syncDateRangeDraft();
    showDateRangePicker = false;
    const promise = loadDashboard("refresh_usage", {
      preserveView: true,
      filters,
      includeDetails: activePage !== "stats",
      background: options.background ?? false
    }).finally(() => {
      refreshPromise = null;
      if (!syncing) {
        syncStatusMessage = "";
      }
    });
    refreshPromise = promise;
    return promise;
  }

  async function applyFilters() {
    normalizeDateRange();
    showDateRangePicker = false;
    await loadDashboard("query_usage", {
      preserveView: false,
      filters,
      includeDetails: activePage !== "stats"
    });
  }

  async function autoApplyFilters(nextFilters: UsageFilters) {
    filters = nextFilters;
    normalizeDateRange();
    syncDateRangeDraft();
    showDateRangePicker = false;
    await loadDashboard("query_usage", {
      preserveView: false,
      filters: { ...filters, search: appliedFilters.search },
      includeDetails: activePage !== "stats"
    });
  }

  async function loadDashboard(
    command: "refresh_usage" | "query_usage",
    options: LoadDashboardOptions
  ) {
    const requestId = ++dashboardRequestSequence;
    const includeDetails = options.includeDetails ?? activePage !== "stats";
    activeDashboardRequest = { id: requestId, includeDetails };
    loading = !options.background;
    syncing = Boolean(options.background);
    error = "";
    const snapshot = options.preserveView ? captureViewSnapshot() : null;
    const requestFilters = {
      ...options.filters,
      onlyAnomalies: false,
      includeDetails
    };
    try {
      const nextData = await invoke<DashboardData>(command, {
        filters: requestFilters
      });
      if (requestId !== dashboardRequestSequence) {
        return;
      }
      if (!nextData.detailsLoaded && activePage !== "stats") {
        await loadDashboard("query_usage", {
          preserveView: options.preserveView,
          filters: options.filters,
          includeDetails: true,
          background: options.background
        });
        return;
      }
      const canCompareRows = command === "refresh_usage" && Boolean(data) && filtersEqual(requestFilters, appliedFilters);
      const changedRows =
        canCompareRows && data
          ? changedDetailRows(data.detailRows, nextData.detailRows)
          : new Set<string>();
      updatedDetailRows = changedRows;
  if (canCompareRows) {
    monitorUserInputKeys = collectMonitorUserInputKeys(monitorUserInputKeys, changedRows, nextData.detailRows);
  }
      data = nextData;
      if (!nextData.detailsLoaded) {
        saveDashboardCache(nextData);
      }
      appliedFilters = requestFilters;
      if (snapshot) {
        await restoreViewSnapshot(snapshot, nextData);
      } else {
        expandedDetailRows = new Set();
        detailExpandLevel = 0;
      }
      settings = normalizeConfig(nextData.config);
      syncSettingsDrafts(settings);
      setupRefreshTimer(settings.refreshIntervalSeconds);
      setTimeout(() => {
        void maybeAutoCheckUpdate();
        void ensureStableEntry();
      }, 0);
      saveUiState();
    } catch (unknownError) {
      if (requestId === dashboardRequestSequence) {
        error = unknownError instanceof Error ? unknownError.message : String(unknownError);
      }
    } finally {
      if (requestId === dashboardRequestSequence) {
        activeDashboardRequest = null;
        loading = false;
        syncing = false;
      }
    }
  }

  function captureViewSnapshot(): ViewSnapshot {
    return {
      expandedDetailRows: [...expandedDetailRows],
      detailExpandLevel,
      windowScrollX: window.scrollX,
      windowScrollY: window.scrollY,
      detailScrollLeft: detailTableWrap?.scrollLeft ?? 0,
      detailScrollTop: detailTableWrap?.scrollTop ?? 0
    };
  }

  async function restoreViewSnapshot(snapshot: ViewSnapshot, nextData: DashboardData) {
    if (snapshot.detailExpandLevel === null) {
      expandedDetailRows = filterExistingKeys(snapshot.expandedDetailRows, nextData.detailRows);
    } else {
      expandedDetailRows = new Set(
        nextData.detailRows
          .filter((row) => row.hasChildren && row.level < snapshot.detailExpandLevel!)
          .map((row) => row.rowKey)
      );
    }
    detailExpandLevel = snapshot.detailExpandLevel;
    await tick();
    if (detailTableWrap) {
      detailTableWrap.scrollLeft = snapshot.detailScrollLeft;
      detailTableWrap.scrollTop = snapshot.detailScrollTop;
    }
    window.scrollTo(snapshot.windowScrollX, snapshot.windowScrollY);
  }

  function filterExistingKeys<T extends { rowKey: string }>(keys: string[], rows: T[]): Set<string> {
    const existing = new Set(rows.map((row) => row.rowKey));
    return new Set(keys.filter((key) => existing.has(key)));
  }

  function changedDetailRows(previousRows: DetailRow[], nextRows: DetailRow[]): Set<string> {
    const previousSignatures = new Map(previousRows.map((row) => [row.rowKey, detailRowSignature(row)]));
    return new Set(
      nextRows
        .filter((row) => previousSignatures.get(row.rowKey) !== detailRowSignature(row))
        .map((row) => row.rowKey)
    );
  }

  function detailRowSignature(row: DetailRow): string {
    return [
      row.parentKey ?? "",
      row.hasChildren,
      row.level,
      row.kind,
      row.node,
      row.startTime,
      row.lastTime,
      row.inputTokens,
      row.cachedInputTokens,
      row.nonCachedInputTokens,
      row.outputTokens,
      row.reasoningOutputTokens,
      row.totalTokens,
      row.status,
      row.statusReason
    ].join("|");
  }

  function filtersEqual(a: UsageFilters, b: UsageFilters): boolean {
    return (
      a.dateFrom === b.dateFrom &&
      a.dateTo === b.dateTo &&
      a.project === b.project &&
      a.session === b.session &&
      a.search === b.search
    );
  }

  function collectMonitorUserInputKeys(
    existingKeys: Set<string>,
    changedRows: Set<string>,
    rows: DetailRow[]
  ): Set<string> {
    if (changedRows.size === 0) return existingKeys;
    const byKey = new Map(rows.map((row) => [row.rowKey, row]));
    const next = new Set(existingKeys);
    for (const key of changedRows) {
      const row = byKey.get(key);
      const inputKey = row ? userInputKeyForRow(row, byKey) : null;
      if (inputKey) {
        next.add(inputKey);
      }
    }
    return next;
  }

  function buildMonitorRows(rows: DetailRow[], userInputKeys: Set<string>, startedAt: Date): DetailRow[] {
    if (userInputKeys.size === 0) return [];
    return rows.filter(
      (row) => row.kind === "UserInput" && userInputKeys.has(row.rowKey) && isAfterMonitorStart(row, startedAt)
    );
  }

  function isAfterMonitorStart(row: DetailRow, startedAt: Date): boolean {
    const rowTime = parseLocalDateTime(row.lastTime || row.time);
    return Boolean(rowTime && rowTime.getTime() >= startedAt.getTime());
  }

  function userInputKeyForRow(row: DetailRow, byKey = detailRowsByKey): string | null {
    let current: DetailRow | undefined = row;
    while (current) {
      if (current.kind === "UserInput") return current.rowKey;
      current = current.parentKey ? byKey.get(current.parentKey) : undefined;
    }
    return null;
  }

  function ancestorOfKind(row: DetailRow, kind: string, byKey = detailRowsByKey): DetailRow | null {
    let current: DetailRow | undefined = row;
    while (current) {
      if (current.kind === kind) return current;
      current = current.parentKey ? byKey.get(current.parentKey) : undefined;
    }
    return null;
  }

  function resetFilters() {
    filters = defaultFilters();
    syncDateRangeDraft();
    showOnlyAnomalies = false;
    showDateRangePicker = false;
    applyFilters();
  }

  function visibleTreeRows<T extends { rowKey: string; parentKey: string | null }>(
    rows: T[],
    expandedRows: Set<string>
  ): T[] {
    const byKey = new Map(rows.map((row) => [row.rowKey, row]));
    return rows.filter((row) => {
      let parentKey = row.parentKey;
      while (parentKey) {
        if (!expandedRows.has(parentKey)) return false;
        parentKey = byKey.get(parentKey)?.parentKey ?? null;
      }
      return true;
    });
  }

  function filterDetailRowsByStatus(rows: DetailRow[], normalStatus: string): DetailRow[] {
    const byKey = new Map(rows.map((row) => [row.rowKey, row]));
    const keepKeys = new Set<string>();
    rows
      .filter((row) => row.status !== normalStatus)
      .forEach((row) => {
        keepKeys.add(row.rowKey);
        let parentKey = row.parentKey;
        while (parentKey) {
          keepKeys.add(parentKey);
          parentKey = byKey.get(parentKey)?.parentKey ?? null;
        }
      });
    return rows.filter((row) => keepKeys.has(row.rowKey));
  }

  function updateDetailSort(key: DetailSortKey) {
    const defaultDirection: SortDirection = key === "node" ? "asc" : "desc";
    detailSort = detailSort.key === key
      ? { key, direction: detailSort.direction === "asc" ? "desc" : "asc" }
      : { key, direction: defaultDirection };
  }

  function updateMonitorSort(key: MonitorSortKey) {
    const defaultDirection: SortDirection = key === "projectSession" ? "asc" : "desc";
    monitorSort = monitorSort.key === key
      ? { key, direction: monitorSort.direction === "asc" ? "desc" : "asc" }
      : { key, direction: defaultDirection };
  }

  function sortIndicator(active: boolean, direction: SortDirection): string {
    return active ? (direction === "asc" ? "↑" : "↓") : "↕";
  }

  function sortDetailTreeRows(
    rows: DetailRow[],
    sort: { key: DetailSortKey; direction: SortDirection }
  ): DetailRow[] {
    const byParent = new Map<string | null, DetailRow[]>();
    rows.forEach((row) => {
      const siblings = byParent.get(row.parentKey) ?? [];
      siblings.push(row);
      byParent.set(row.parentKey, siblings);
    });

    const ordered: DetailRow[] = [];
    const appendChildren = (parentKey: string | null) => {
      const siblings = [...(byParent.get(parentKey) ?? [])].sort((left, right) => compareDetailRows(left, right, sort));
      siblings.forEach((row) => {
        ordered.push(row);
        appendChildren(row.rowKey);
      });
    };
    appendChildren(null);
    return ordered;
  }

  function compareDetailRows(
    left: DetailRow,
    right: DetailRow,
    sort: { key: DetailSortKey; direction: SortDirection }
  ): number {
    let result = 0;
    if (sort.key === "node") {
      result = detailNodeDisplayText(left).localeCompare(detailNodeDisplayText(right), "zh-CN", {
        numeric: true,
        sensitivity: "base"
      });
    } else if (sort.key === "totalTokens") {
      result = left.totalTokens - right.totalTokens;
    } else {
      result = detailSortTime(left, sort.key).localeCompare(detailSortTime(right, sort.key));
    }
    if (result === 0) result = left.rowKey.localeCompare(right.rowKey);
    return sort.direction === "asc" ? result : -result;
  }

  function detailSortTime(row: DetailRow, key: "lastTime" | "startTime"): string {
    const value = key === "startTime" ? row.startTime || row.time : row.lastTime || row.time;
    return value.replace(/\D/g, "");
  }

  function sortMonitorRows(
    rows: DetailRow[],
    sort: { key: MonitorSortKey; direction: SortDirection }
  ): DetailRow[] {
    return [...rows].sort((left, right) => {
      let result = 0;
      if (sort.key === "projectSession") {
        result = detailProjectSessionText(left).localeCompare(detailProjectSessionText(right), "zh-CN", {
          numeric: true,
          sensitivity: "base"
        });
      } else if (sort.key === "totalTokens") {
        result = left.totalTokens - right.totalTokens;
      } else {
        result = detailSortTime(left, "lastTime").localeCompare(detailSortTime(right, "lastTime"));
      }
      if (result === 0) result = left.rowKey.localeCompare(right.rowKey);
      return sort.direction === "asc" ? result : -result;
    });
  }

  function filterSessionOptions(options: FilterOption[], selectedProject: string): FilterOption[] {
    if (!selectedProject) return options;
    return options.filter((option) => option.project === selectedProject);
  }

  function sessionOptionExists(options: FilterOption[], value: string): boolean {
    return options.some((option) => option.value === value);
  }

  async function handleProjectSelect(event: Event) {
    const project = (event.currentTarget as HTMLSelectElement).value;
    const sessionsForProject = filterSessionOptions(data?.sessionOptions ?? [], project);
    await autoApplyFilters({
      ...filters,
      project,
      session: filters.session && sessionOptionExists(sessionsForProject, filters.session) ? filters.session : ""
    });
  }

  async function handleSessionSelect(event: Event) {
    await autoApplyFilters({
      ...filters,
      session: (event.currentTarget as HTMLSelectElement).value
    });
  }

  function toggleDetailRow(rowKey: string) {
    const next = new Set(expandedDetailRows);
    if (next.has(rowKey)) {
      next.delete(rowKey);
    } else {
      next.add(rowKey);
    }
    expandedDetailRows = next;
    detailExpandLevel = null;
  }

  function expandDetailToLevel(level: number) {
    const keys = detailRows
      .filter((row) => row.hasChildren && row.level < level)
      .map((row) => row.rowKey);
    expandedDetailRows = new Set(keys);
    detailExpandLevel = level;
  }

  function toggleOnlyAnomalies() {
    const nextShowOnlyAnomalies = !showOnlyAnomalies;
    showOnlyAnomalies = nextShowOnlyAnomalies;
    if (detailExpandLevel !== null) {
      const sourceRows = nextShowOnlyAnomalies
        ? filterDetailRowsByStatus(data?.detailRows ?? [], "正常")
        : data?.detailRows ?? [];
      const keys = sourceRows
        .filter((row) => row.hasChildren && row.level < detailExpandLevel!)
        .map((row) => row.rowKey);
      expandedDetailRows = new Set(keys);
    }
  }

  function setPage(page: Page) {
    activePage = page;
    saveUiState();
    const needsDetails =
      page !== "stats" && (!data?.detailsLoaded || activeDashboardRequest?.includeDetails === false);
    if (needsDetails) {
      void loadDashboard("query_usage", {
        preserveView: false,
        filters: appliedFilters,
        includeDetails: true
      });
    }
  }

  function toggleDateRangePicker() {
    showDateRangePicker = !showDateRangePicker;
    if (showDateRangePicker) {
      syncDateRangeDraft();
      syncCalendarBase();
      rangeAnchor = null;
    }
  }

  async function setRecentRange(days: number) {
    const today = parseDateValue(todayLocalDate()) ?? new Date();
    draftDateFrom = formatDateValue(addDays(today, -(days - 1)));
    draftDateTo = formatDateValue(today);
    syncCalendarBase();
    rangeAnchor = null;
    await autoApplyFilters({ ...filters, dateFrom: draftDateFrom, dateTo: draftDateTo });
  }

  function startEditSessionsRoot() {
    draftSessionsRoot = settings.sessionsRoot;
    editingSessionsRoot = true;
  }

  function cancelEditSessionsRoot() {
    draftSessionsRoot = settings.sessionsRoot;
    editingSessionsRoot = false;
  }

  function startEditRefreshInterval() {
    draftRefreshIntervalSeconds = settings.refreshIntervalSeconds;
    editingRefreshInterval = true;
  }

  function cancelEditRefreshInterval() {
    draftRefreshIntervalSeconds = settings.refreshIntervalSeconds;
    editingRefreshInterval = false;
  }

  async function saveAppConfig(nextConfig: AppConfig, options: { refreshAfter: boolean }) {
    settingsMessage = "";
    exportMessage = "";
    error = "";
    try {
      settings = normalizeConfig(
        await invoke<AppConfig>("save_settings", {
          config: normalizeConfig(nextConfig)
        })
      );
      syncSettingsDrafts(settings);
      setupRefreshTimer(settings.refreshIntervalSeconds);
      settingsMessage = "设置已保存";
      if (options.refreshAfter) {
        await refreshUsage();
      }
    } catch (unknownError) {
      error = unknownError instanceof Error ? unknownError.message : String(unknownError);
    }
  }

  async function saveSessionsRoot() {
    const nextSessionsRoot = draftSessionsRoot.trim();
    if (!nextSessionsRoot) return;
    editingSessionsRoot = false;
    await saveAppConfig({ ...settings, sessionsRoot: nextSessionsRoot }, { refreshAfter: true });
  }

  async function saveRefreshInterval() {
    const nextInterval = Math.max(0, Number(draftRefreshIntervalSeconds) || 0);
    draftRefreshIntervalSeconds = nextInterval;
    editingRefreshInterval = false;
    await saveAppConfig({ ...settings, refreshIntervalSeconds: nextInterval }, { refreshAfter: false });
  }

  async function checkUpdate(options: { autoInstall?: boolean } = {}) {
    if (updateButtonStatus === "checking" || updateButtonStatus === "downloading" || updateButtonStatus === "installing") {
      return;
    }
    const autoInstall = options.autoInstall ?? true;
    updateResultMessage = "正在检查更新...";
    updateDownloadProgress = null;
    setUpdateButtonStatus("checking");
    error = "";
    try {
      updateInfo = await invoke<UpdateInfo>("check_update", { source: DEFAULT_UPDATE_SOURCE });
      if (updateInfo.hasUpdate && updateInfo.downloadUrl) {
        updateResultMessage = `发现新版本 v${updateInfo.latestVersion ?? ""}，正在自动下载并安装...`;
        if (autoInstall) {
          await installAvailableUpdate();
        } else {
          updateResultMessage = `发现新版本 v${updateInfo.latestVersion ?? ""}，点击“立即更新”开始安装。`;
          setUpdateButtonStatus("ready");
        }
      } else if (updateInfo.hasUpdate) {
        updateResultMessage = `发现新版本 v${updateInfo.latestVersion ?? ""}，但暂无可用安装包。`;
        setTemporaryUpdateStatus("failed");
      } else {
        updateResultMessage = `当前已是最新版本 v${updateInfo.currentVersion}。`;
        setTemporaryUpdateStatus("latest");
      }
    } catch {
      updateInfo = null;
      updateResultMessage = "检查更新失败，请检查网络连接或更新源后重试。";
      setTemporaryUpdateStatus("failed");
    }
  }

  async function installAvailableUpdate() {
    if (!updateInfo?.downloadUrl) {
      updateResultMessage = "没有可用的更新安装包。";
      setTemporaryUpdateStatus("failed");
      return;
    }
    setUpdateButtonStatus("downloading");
    updateDownloadProgress = null;
    updateResultMessage = `正在下载 v${updateInfo.latestVersion ?? "新版本"}...`;
    error = "";
    try {
      const result = await invoke<DownloadUpdateResult>("download_update_package", {
        input: {
          downloadUrl: updateInfo.downloadUrl,
          version: updateInfo.latestVersion
        }
      });
      setUpdateButtonStatus("installing");
      updateResultMessage = "下载完成，正在安装更新，软件即将重启...";
      await invoke<InstallDownloadedUpdateResult>("install_downloaded_update", {
        input: { packagePath: result.path }
      });
    } catch {
      updateResultMessage = "更新失败，下载或安装未完成，请稍后重试。";
      setTemporaryUpdateStatus("failed");
    }
  }

  async function loadUpdateRuntimeInfo() {
    try {
      updateRuntimeInfo = await invoke<UpdateRuntimeInfo>("get_update_runtime_info");
    } catch {
      updateRuntimeInfo = null;
    }
  }

  async function ensureStableEntry() {
    if (stableEntryEnsured) return;
    stableEntryEnsured = true;
    if (import.meta.env.DEV) {
      await loadUpdateRuntimeInfo();
      return;
    }
    try {
      await invoke<InstallStableEntryResult>("install_stable_entry");
      await loadUpdateRuntimeInfo();
    } catch {
      await loadUpdateRuntimeInfo();
    }
  }

  function maybeAutoCheckUpdate() {
    if (autoUpdateChecked) return;
    autoUpdateChecked = true;
    void checkUpdate();
  }

  function setUpdateButtonStatus(status: typeof updateButtonStatus) {
    updateButtonStatus = status;
  }

  function setTemporaryUpdateStatus(status: "latest" | "failed") {
    setUpdateButtonStatus(status);
  }

  function updateButtonLabel(): string {
    if (updateButtonStatus === "downloading") {
      const progress = updateDownloadProgress;
      if (progress?.totalBytes && progress.totalBytes > 0) {
        return `下载中 ${formatBytes(progress.downloadedBytes)} / ${formatBytes(progress.totalBytes)}`;
      }
      if (progress && progress.downloadedBytes > 0) {
        return `下载中 ${formatBytes(progress.downloadedBytes)}`;
      }
      return "下载中";
    }
    if (updateButtonStatus === "failed") {
      return updateResultMessage.startsWith("检查更新失败") ? "检查失败" : "更新失败";
    }
    const labels: Record<Exclude<typeof updateButtonStatus, "downloading" | "failed">, string> = {
      idle: "检查更新",
      checking: "检查中",
      latest: "已是最新",
      ready: "立即更新",
      installing: "安装中"
    };
    return labels[updateButtonStatus];
  }

  function updateButtonDisabled(): boolean {
    return loading || updateButtonStatus === "checking" || updateButtonStatus === "downloading" || updateButtonStatus === "installing";
  }

  async function handleUpdateButton() {
    await checkUpdate({ autoInstall: true });
  }

  async function exportDetail() {
    await exportCsv("export_detail_csv");
  }

  async function exportCsv(command: "export_detail_csv") {
    exportMessage = "";
    settingsMessage = "";
    error = "";
    try {
      const result = await invoke<ExportResult>(command, { filters: appliedFilters });
      exportMessage = `已导出 ${formatCount(result.rowCount)} 行：${result.path}`;
    } catch (unknownError) {
      error = unknownError instanceof Error ? unknownError.message : String(unknownError);
    }
  }

  async function rebuildLedger() {
    if (!confirm("重建只会清空本工具的本地数据库，并会先备份 SQLite 文件；不会删除 Codex 原始日志。确认继续？")) {
      return;
    }
    settingsMessage = "";
    exportMessage = "";
    error = "";
    try {
      const result = await invoke<RebuildResult>("rebuild_ledger");
      settingsMessage = result.backupPath ? `数据库已重建，备份：${result.backupPath}` : "数据库已重建";
      data = data ? { ...data, scanState: result.scanState } : data;
      await refreshUsage();
    } catch (unknownError) {
      error = unknownError instanceof Error ? unknownError.message : String(unknownError);
    }
  }

  function setupRefreshTimer(seconds: number) {
    clearRefreshTimer();
    if (seconds > 0) {
      refreshTimer = setInterval(() => {
        refreshUsage();
      }, seconds * 1000);
    }
  }

  function clearRefreshTimer() {
    if (refreshTimer) {
      clearInterval(refreshTimer);
      refreshTimer = null;
    }
  }

  function restoreUiState() {
    try {
      localStorage.removeItem("codex-token-usage-filters");
      filters = defaultFilters();
      appliedFilters = { ...filters };
    } catch {
      filters = defaultFilters();
      appliedFilters = { ...filters };
    }
  }

  function restoreDashboardCache(): DashboardData | null {
    try {
      const raw = localStorage.getItem(DASHBOARD_CACHE_KEY);
      if (!raw) return null;
      const cached = JSON.parse(raw) as DashboardData;
      if (!cached || !cached.metrics || !cached.config || !cached.scanState) return null;
      return {
        ...cached,
        detailRows: [],
        summaryRows: [],
        detailsLoaded: false,
        performanceTimings: cached.performanceTimings ?? { queryMs: 0, assemblyMs: 0, totalMs: 0 }
      };
    } catch {
      return null;
    }
  }

  function saveDashboardCache(nextData: DashboardData) {
    try {
      const cached = {
        ...nextData,
        detailRows: [],
        summaryRows: [],
        detailsLoaded: false
      };
      localStorage.setItem(DASHBOARD_CACHE_KEY, JSON.stringify(cached));
    } catch {
      // Cache is an optional startup optimization.
    }
  }

  function saveUiState() {
    try {
      localStorage.removeItem("codex-token-usage-filters");
    } catch {
      // ignore storage failures
    }
  }

  function formatToken(value: number): string {
    if (!Number.isFinite(value)) return "0";
    if (value < 10000) return Math.round(value).toString();
    if (value >= 100000000) return `${trimUnit(value / 100000000, 2)}亿`;
    return `${trimUnit(value / 10000, 2)}万`;
  }

  function trimUnit(value: number, decimals: number): string {
    return value
      .toFixed(decimals)
      .replace(/\.0+$/, "")
      .replace(/(\.\d*?)0+$/, "$1");
  }

  function formatCount(value: number): string {
    return new Intl.NumberFormat("zh-CN").format(value);
  }

  function formatBytes(value: number): string {
    if (!Number.isFinite(value) || value < 1024) return `${Math.max(0, Math.round(value || 0))} B`;
    const units = ["KB", "MB", "GB"];
    let size = value;
    let unitIndex = -1;
    while (size >= 1024 && unitIndex < units.length - 1) {
      size /= 1024;
      unitIndex += 1;
    }
    return `${size.toFixed(size >= 10 ? 0 : 1)} ${units[unitIndex]}`;
  }

  function statusClass(status: string): string {
    if (status === "超高" || status === "异常") return "status-bad";
    if (status === "偏高") return "status-warn";
    return "status-ok";
  }

  function totalToneClass(status: string): string {
    if (status === "超高" || status === "异常") return "total-bad";
    if (status === "偏高") return "total-warn";
    return "total-ok";
  }

  function levelLabel(level: number): string {
    return ["P", "S", "I", "T", "#"][level] ?? "-";
  }

  function detailKindLabel(kind: string): string {
    const labels: Record<string, string> = {
      Project: "项目",
      Session: "会话",
      UserInput: "用户输入",
      Turn: "轮次",
      TokenCount: "Token记录"
    };
    return labels[kind] ?? kind;
  }

  function detailNodeText(row: DetailRow): string {
    if (row.kind === "UserInput") return row.node.replace(/^用户输入\s*/, "");
    if (row.kind === "Turn") return row.node.replace(/^Turn\s*/, "");
    if (row.kind === "TokenCount") return row.node.replace(/^TokenCount\s*/, "");
    return row.node;
  }

  function detailNodeDisplayText(row: DetailRow): string {
    if (row.kind !== "UserInput") return detailNodeText(row);
    const fullText = row.nodeTooltip.replace(/^用户输入\s*\n?/, "").trim();
    const actualInput = extractActualUserInput(fullText);
    return actualInput ? compactOneLine(actualInput) : detailNodeText(row);
  }

  function detailTooltipText(row: DetailRow): string {
    if (row.kind !== "TokenCount") return row.nodeTooltip;
    return row.nodeTooltip.replace(/\n?完整用户输入[:：][\s\S]*$/u, "").trim();
  }

  function extractActualUserInput(text: string): string {
    const match = text.match(/(?:^|\n)#{1,6}\s*My request for Codex:\s*/i);
    if (!match || match.index === undefined) return "";
    return text.slice(match.index + match[0].length).trim();
  }

  function compactOneLine(text: string): string {
    return text.replace(/\s+/g, " ").trim();
  }

  function detailProjectSessionText(row: DetailRow): string {
    const projectRow = ancestorOfKind(row, "Project");
    const sessionRow = ancestorOfKind(row, "Session");
    const projectName = projectRow ? detailNodeText(projectRow) : row.project || "-";
    const sessionName = sessionRow ? detailNodeText(sessionRow) : row.sessionId || "-";
    return `${projectName}/${sessionName}`;
  }

  function showTooltip(event: MouseEvent | FocusEvent, text: string) {
    cancelTooltipHide();
    const value = text.trim();
    if (!value) return;
    const target = event.currentTarget as HTMLElement;
    const rect = target.getBoundingClientRect();
    const width = Math.min(760, window.innerWidth - 24);
    const x = Math.max(12, Math.min(rect.left, window.innerWidth - width - 12));
    const maxTooltipHeight = Math.min(386, window.innerHeight - 24);
    const spaceBelow = window.innerHeight - rect.bottom - 12;
    const spaceAbove = rect.top - 12;
    const shouldShowAbove = spaceBelow < maxTooltipHeight && spaceAbove > spaceBelow;
    tooltip = {
      visible: true,
      text: value,
      x,
      y: shouldShowAbove ? rect.top - 8 : rect.bottom + 8,
      placement: shouldShowAbove ? "above" : "below"
    };
  }

  function cancelTooltipHide() {
    if (tooltipHideTimer) {
      clearTimeout(tooltipHideTimer);
      tooltipHideTimer = null;
    }
  }

  function scheduleHideTooltip() {
    cancelTooltipHide();
    tooltipHideTimer = setTimeout(() => {
      hideTooltip();
    }, 120);
  }

  function hideTooltip() {
    cancelTooltipHide();
    tooltip = { ...tooltip, visible: false };
  }

  function buildHourlyDays(currentFilters: UsageFilters): HourlyDay[] {
    return buildRecentWindowDays(currentFilters, 14);
  }

  function buildHourlyMap(buckets: HourlyBucket[]): Map<string, HourlyBucket> {
    return new Map(buckets.map((bucket) => [`${bucket.date}|${bucket.hour}`, bucket]));
  }

  function buildRecentWindowDays(currentFilters: UsageFilters, windowDays: number): HourlyDay[] {
    const endDate = parseDateValue(currentFilters.dateTo) ?? parseDateValue(todayLocalDate()) ?? new Date();
    return Array.from({ length: windowDays }, (_, index) => {
      const date = formatDateValue(addDays(endDate, index - (windowDays - 1)));
      return {
        date,
        inRange: dateWithinFilters(date, currentFilters)
      };
    });
  }

  function buildDailyTrendBuckets(currentFilters: UsageFilters, buckets: TrendBucket[]): DailyTrendBucket[] {
    const byDate = new Map(buckets.map((bucket) => [bucket.label, bucket]));
    return buildRecentWindowDays(currentFilters, 14).map((day) => {
      const bucket = day.inRange ? byDate.get(day.date) : undefined;
      return {
        ...(bucket ?? {
          label: day.date,
          totalTokens: 0,
          status: "正常"
        }),
        label: day.date,
        inRange: day.inRange,
        displayLabel: day.inRange ? shortDateLabel(day.date) : "-"
      };
    });
  }

  function buildMonthlyTrendBuckets(currentFilters: UsageFilters, buckets: TrendBucket[]): MonthlyTrendBucket[] {
    return buckets.map((bucket) => {
      const inRange = monthWithinFilters(bucket.label, currentFilters);
      return {
        ...bucket,
        inRange,
        displayLabel: inRange ? bucket.label : "-"
      };
    });
  }

  function dateWithinFilters(date: string, currentFilters: UsageFilters): boolean {
    const from = currentFilters.dateFrom || "0000-00-00";
    const to = currentFilters.dateTo || "9999-99-99";
    return date >= from && date <= to;
  }

  function monthWithinFilters(month: string, currentFilters: UsageFilters): boolean {
    const monthStart = parseDateValue(`${month}-01`);
    if (!monthStart) return false;
    const monthEnd = formatDateValue(addDays(addMonths(monthStart, 1), -1));
    const from = currentFilters.dateFrom || "0000-00-00";
    const to = currentFilters.dateTo || "9999-99-99";
    return monthEnd >= from && `${month}-01` <= to;
  }

  function bucketClass(bucket: HourlyBucket | undefined): string {
    if (!bucket || bucket.totalTokens <= 0) return "heat-cell empty";
    const ratio = hourlyPeak <= 0 ? 0 : bucket.totalTokens / hourlyPeak;
    if (ratio >= 0.82) return "heat-cell strong";
    if (ratio >= 0.66) return "heat-cell high";
    if (ratio >= 0.33) return "heat-cell medium";
    return "heat-cell low";
  }

  function hasTrendData(bucket: TrendBucket): boolean {
    return bucket.totalTokens > 0;
  }

  function trendValueText(bucket: TrendBucket, compact = false): string {
    if (!hasTrendData(bucket)) return "-";
    return compact ? formatCompactToken(bucket.totalTokens) : formatToken(bucket.totalTokens);
  }

  function rangedTrendValueText(bucket: TrendBucket, inRange: boolean, compact = false): string {
    if (!inRange) return "-";
    if (!hasTrendData(bucket)) return "0";
    return compact ? formatCompactToken(bucket.totalTokens) : formatToken(bucket.totalTokens);
  }

  function formatCompactToken(value: number): string {
    if (!Number.isFinite(value) || value <= 0) return "-";
    if (value < 10000) return Math.round(value).toString();
    if (value >= 100000000) return `${trimUnit(value / 100000000, 1)}亿`;
    return `${trimUnit(value / 10000, 0)}万`;
  }

  function trendHeight(bucket: TrendBucket, peak: number): string {
    if (peak <= 0 || !hasTrendData(bucket)) return "0%";
    return `${Math.max((bucket.totalTokens / peak) * 100, 2)}%`;
  }

  function barWidth(value: number, peak: number): string {
    if (peak <= 0) return "0%";
    return `${Math.max((value / peak) * 100, 2)}%`;
  }

  function trendClass(bucket: TrendBucket, peak: number): string {
    if (!hasTrendData(bucket)) return "";
    const ratio = peak <= 0 ? 0 : bucket.totalTokens / peak;
    if (ratio >= 0.82) return "tone-abnormal";
    if (ratio >= 0.66) return "tone-high";
    if (ratio >= 0.33) return "tone-medium";
    return "tone-low";
  }

  function rankToneClass(value: number, peak: number): string {
    const ratio = peak <= 0 ? 0 : value / peak;
    if (ratio >= 0.82) return "tone-abnormal";
    if (ratio >= 0.66) return "tone-high";
    if (ratio >= 0.33) return "tone-medium";
    return "tone-low";
  }

  function dayLabel(date: string): string {
    return shortDateLabel(date);
  }

  function shortDateLabel(date: string): string {
    return date.slice(5).replace("-", "/");
  }

  function formatDisplayDate(date: string): string {
    return date.replaceAll("-", "/");
  }

  function dateRangeText(dateFrom: string, dateTo: string): string {
    return `${formatDisplayDate(dateFrom)} 至 ${formatDisplayDate(dateTo)}`;
  }

  function buildCalendarMonth(monthDate: Date, selectedFrom: string, selectedTo: string): CalendarMonth {
    const monthStart = startOfMonth(monthDate);
    const weekOffset = (monthStart.getDay() + 6) % 7;
    const gridStart = addDays(monthStart, -weekOffset);
    const currentMonth = monthStart.getMonth();
    const today = todayLocalDate();
    return {
      label: `${monthStart.getFullYear()}年 ${monthStart.getMonth() + 1}月`,
      days: Array.from({ length: 42 }, (_, index) => {
        const date = addDays(gridStart, index);
        const value = formatDateValue(date);
        return {
          value,
          label: date.getDate(),
          inCurrentMonth: date.getMonth() === currentMonth,
          isToday: value === today,
          isStart: date.getMonth() === currentMonth && value === selectedFrom,
          isEnd: date.getMonth() === currentMonth && value === selectedTo,
          inRange: date.getMonth() === currentMonth && value > selectedFrom && value < selectedTo
        };
      })
    };
  }

  function calendarDayClass(day: CalendarDay): string {
    return [
      "calendar-day",
      day.inCurrentMonth ? "" : "muted",
      day.inRange ? "in-range" : "",
      day.isStart ? "range-start" : "",
      day.isEnd ? "range-end" : "",
      day.isToday ? "today" : ""
    ]
      .filter(Boolean)
      .join(" ");
  }

  function selectRangeDate(value: string) {
    if (!rangeAnchor) {
      draftDateFrom = value;
      draftDateTo = value;
      rangeAnchor = value;
      return;
    }
    if (value < rangeAnchor) {
      draftDateFrom = value;
      draftDateTo = rangeAnchor;
    } else {
      draftDateFrom = rangeAnchor;
      draftDateTo = value;
    }
    rangeAnchor = null;
  }

  function moveCalendar(months: number) {
    calendarBaseMonth = addMonths(calendarBaseMonth, months);
  }

  function syncCalendarBase() {
    calendarBaseMonth = startOfMonth(parseDateValue(draftDateFrom) ?? new Date());
  }

  function syncDateRangeDraft() {
    draftDateFrom = filters.dateFrom;
    draftDateTo = filters.dateTo;
  }

  async function confirmDateRange() {
    rangeAnchor = null;
    await autoApplyFilters({ ...filters, dateFrom: draftDateFrom, dateTo: draftDateTo });
  }

  function formatDetailStartTime(row: DetailRow): string {
    if (row.kind === "TokenCount") return "";
    return formatLocalMinute(row.startTime || row.time);
  }

  function formatDetailLastTime(row: DetailRow): string {
    return formatLocalMinute(row.lastTime || row.time);
  }

  function formatLocalMinute(value: string): string {
    const cleanValue = value.trim().replace(/\s+[+-]\d{2}:\d{2}$/, "");
    if (cleanValue.length < 16) return cleanValue.replaceAll("-", "/");
    return cleanValue.slice(0, 16).replaceAll("-", "/");
  }

  function parseLocalDateTime(value: string): Date | null {
    const trimmed = value.trim();
    if (!trimmed) return null;
    const withOffset = trimmed.replaceAll("/", "-").replace(/\s+([+-]\d{2}:\d{2})$/, "$1").replace(" ", "T");
    const parsed = new Date(withOffset);
    if (!Number.isNaN(parsed.getTime())) return parsed;
    const withoutOffset = trimmed.replaceAll("/", "-").replace(/\s+[+-]\d{2}:\d{2}$/, "").replace(" ", "T");
    const fallback = new Date(withoutOffset);
    return Number.isNaN(fallback.getTime()) ? null : fallback;
  }

  function normalizeDateRange() {
    const from = parseDateValue(filters.dateFrom);
    const to = parseDateValue(filters.dateTo);
    if (!from || !to || from <= to) return;
    filters = {
      ...filters,
      dateFrom: formatDateValue(to),
      dateTo: formatDateValue(from)
    };
  }

  function parseDateValue(value: string): Date | null {
    const parsed = new Date(`${value}T00:00:00`);
    return Number.isNaN(parsed.getTime()) ? null : parsed;
  }

  function addDays(date: Date, days: number): Date {
    const next = new Date(date);
    next.setDate(next.getDate() + days);
    return next;
  }

  function addMonths(date: Date, months: number): Date {
    const next = new Date(date);
    next.setMonth(next.getMonth() + months, 1);
    return next;
  }

  function startOfMonth(date: Date): Date {
    return new Date(date.getFullYear(), date.getMonth(), 1);
  }

  function formatDateValue(date: Date): string {
    const month = `${date.getMonth() + 1}`.padStart(2, "0");
    const day = `${date.getDate()}`.padStart(2, "0");
    return `${date.getFullYear()}-${month}-${day}`;
  }

  function percentOf(value: number, total: number): string {
    if (!Number.isFinite(value) || !Number.isFinite(total) || total <= 0) return "0%";
    return `${trimUnit((value / total) * 100, 1)}%`;
  }

  type RingTheme = {
    color: string;
  };

  const ringThemes = {
    low: { color: "var(--chart-blue-low)" },
    medium: { color: "var(--chart-blue-medium)" },
    high: { color: "var(--chart-blue-high)" },
    strong: { color: "var(--chart-blue-strong)" }
  } satisfies Record<string, RingTheme>;

  function ringSegmentStyle(segments: Array<{ value: number; theme: RingTheme }>, total: number): string {
    if (total <= 0) return "conic-gradient(#eef3f9 0deg 360deg)";
    let cursor = 0;
    const parts = segments.filter((segment) => segment.value > 0).map((segment) => {
      const span = Math.max((segment.value / total) * 360, 0);
      const start = cursor;
      cursor += span;
      return `${segment.theme.color} ${start}deg ${cursor}deg`;
    });
    if (cursor < 360) {
      parts.push(`#eef3f9 ${cursor}deg 360deg`);
    }
    return `conic-gradient(${parts.join(", ")})`;
  }

  function outputTotal(metricValues: Metrics): number {
    return metricValues.outputTokens + metricValues.reasoningOutputTokens;
  }

  function totalRingStyle(metricValues: Metrics): string {
    return ringSegmentStyle(
      [
        { value: metricValues.inputTokens, theme: ringThemes.strong },
        { value: outputTotal(metricValues), theme: ringThemes.medium }
      ],
      metricValues.totalTokens
    );
  }

  function inputRingStyle(metricValues: Metrics): string {
    return ringSegmentStyle(
      [
        { value: metricValues.cachedInputTokens, theme: ringThemes.high },
        { value: metricValues.nonCachedInputTokens, theme: ringThemes.low }
      ],
      metricValues.inputTokens
    );
  }

  function outputRingStyle(metricValues: Metrics): string {
    return ringSegmentStyle(
      [
        { value: metricValues.reasoningOutputTokens, theme: ringThemes.medium },
        { value: metricValues.outputTokens, theme: ringThemes.low }
      ],
      outputTotal(metricValues)
    );
  }

  function timeText(value: string | null | undefined): string {
    return value ? value.replace("T", " ").replace("Z", " UTC") : "-";
  }
</script>

<main class="app-shell" on:contextmenu|preventDefault>
  <header class="topbar">
    <div>
      <h1>Codex Token Usage</h1>
      <p>Codex Token 消耗统计分析看板</p>
    </div>
    <div class="toolbar-controls topbar-controls">
      <label class="toolbar-field refresh-field">
        <div class="toolbar-input-group">
          <span class="toolbar-input-title">自动刷新(s)</span>
          <input
            type="number"
            min="0"
            step="30"
            bind:value={draftRefreshIntervalSeconds}
            readonly={!editingRefreshInterval}
          />
          {#if editingRefreshInterval}
            <button
              type="button"
              class="icon-button primary"
              title="保存自动刷新秒数"
              aria-label="保存自动刷新秒数"
              on:click={saveRefreshInterval}
              disabled={loading}
            >
              ✓
            </button>
            <button
              type="button"
              class="icon-button"
              title="取消编辑自动刷新秒数"
              aria-label="取消编辑自动刷新秒数"
              on:click={cancelEditRefreshInterval}
              disabled={loading}
            >
              ×
            </button>
          {:else}
            <button
              type="button"
              class="icon-button edit-button"
              title="编辑自动刷新秒数"
              aria-label="编辑自动刷新秒数"
              on:click={startEditRefreshInterval}
              disabled={loading}
            >
              <svg class="edit-icon" viewBox="0 0 24 24" aria-hidden="true">
                <path d="M4 20h4L18.5 9.5a2.1 2.1 0 0 0-3-3L5 17v3z" />
                <path d="M14 7l3 3" />
              </svg>
            </button>
          {/if}
        </div>
      </label>
      <label class="toolbar-field session-root-field">
        <div class="toolbar-input-group">
          <span class="toolbar-input-title">会话目录</span>
          <input
            type="text"
            bind:value={draftSessionsRoot}
            readonly={!editingSessionsRoot}
            title={draftSessionsRoot}
          />
          {#if editingSessionsRoot}
            <button
              type="button"
              class="icon-button primary"
              title="保存会话目录"
              aria-label="保存会话目录"
              on:click={saveSessionsRoot}
              disabled={loading || !draftSessionsRoot.trim()}
            >
              ✓
            </button>
            <button
              type="button"
              class="icon-button"
              title="取消编辑会话目录"
              aria-label="取消编辑会话目录"
              on:click={cancelEditSessionsRoot}
              disabled={loading}
            >
              ×
            </button>
          {:else}
            <button
              type="button"
              class="icon-button edit-button"
              title="编辑会话目录"
              aria-label="编辑会话目录"
              on:click={startEditSessionsRoot}
              disabled={loading}
            >
              <svg class="edit-icon" viewBox="0 0 24 24" aria-hidden="true">
                <path d="M4 20h4L18.5 9.5a2.1 2.1 0 0 0-3-3L5 17v3z" />
                <path d="M14 7l3 3" />
              </svg>
            </button>
          {/if}
        </div>
      </label>
      <button class="danger" on:click={rebuildLedger} disabled={!data || loading}>重建数据库</button>
    </div>
  </header>

  {#if error}
    <section class="alert">{error}</section>
  {/if}

  {#if data?.scanState.error}
    <section class="alert">{data.scanState.error}</section>
  {/if}

  {#if settingsMessage || exportMessage}
    <section class="notice">{settingsMessage || exportMessage}</section>
  {/if}

  <section class="view-toolbar">
    <nav aria-label="页面">
      <button class:active={activePage === "stats"} on:click={() => setPage("stats")}>
        统计看板
      </button>
      <button class:active={activePage === "detail"} on:click={() => setPage("detail")}>
        全量明细
      </button>
      <button class:active={activePage === "monitor"} on:click={() => setPage("monitor")}>
        实时监控
      </button>
    </nav>
    <section class="filters header-filters">
      <div class="date-range-filter">
        <button type="button" class="date-range-button" aria-label="日期范围" on:click={toggleDateRangePicker}>
          <span>
            {showDateRangePicker
              ? dateRangeText(draftDateFrom, draftDateTo)
              : dateRangeText(filters.dateFrom, filters.dateTo)}
          </span>
          <svg class="date-range-icon" viewBox="0 0 24 24" aria-hidden="true">
            <rect x="3.5" y="5" width="17" height="15.5" rx="2" />
            <path d="M7 3.5v3M17 3.5v3M4 9h16" />
          </svg>
        </button>
        {#if showDateRangePicker}
          <div class="date-range-popover">
            <div class="range-shortcuts">
              <button type="button" on:click={() => setRecentRange(7)}>最近一周</button>
              <button type="button" on:click={() => setRecentRange(30)}>最近一个月</button>
              <button type="button" on:click={() => setRecentRange(90)}>最近三个月</button>
            </div>
            <div class="range-calendar-toolbar">
              <button type="button" aria-label="上一年" on:click={() => moveCalendar(-12)}>«</button>
              <button type="button" aria-label="上一月" on:click={() => moveCalendar(-1)}>‹</button>
              <span>{dateRangeText(draftDateFrom, draftDateTo)}</span>
              <button type="button" aria-label="下一月" on:click={() => moveCalendar(1)}>›</button>
              <button type="button" aria-label="下一年" on:click={() => moveCalendar(12)}>»</button>
            </div>
            <div class="range-calendars">
              {#each calendarMonths as month}
                <section class="calendar-month">
                  <h3>{month.label}</h3>
                  <div class="calendar-weekdays">
                    {#each ["一", "二", "三", "四", "五", "六", "日"] as weekday}
                      <span>{weekday}</span>
                    {/each}
                  </div>
                  <div class="calendar-days">
                    {#each month.days as day}
                      <button
                        type="button"
                        class={calendarDayClass(day)}
                        on:click={() => selectRangeDate(day.value)}
                        aria-label={day.value}
                      >
                        {day.label}
                      </button>
                    {/each}
                  </div>
                </section>
              {/each}
            </div>
            <div class="range-actions">
              <button
                type="button"
                class="primary"
                on:click={confirmDateRange}
              >
                确定
              </button>
            </div>
          </div>
        {/if}
      </div>
      <label>
        <select value={filters.project} aria-label="项目" on:change={handleProjectSelect}>
          <option value="">全部项目</option>
          {#each data?.projectOptions ?? [] as project}
            <option value={project.value} title={project.title}>{project.label}</option>
          {/each}
        </select>
      </label>
      <label>
        <select value={filters.session} aria-label="会话" on:change={handleSessionSelect}>
          <option value="">全部会话</option>
          {#each filteredSessionOptions as session}
            <option value={session.value} title={session.title}>{session.label}</option>
          {/each}
        </select>
      </label>
      <label class="search">
        <div class="search-control">
          <input type="search" aria-label="搜索" placeholder="项目名称 / 会话名称 / 输入内容" bind:value={filters.search} />
        </div>
      </label>
      <button class="ghost" on:click={resetFilters} disabled={loading}>重置</button>
      <button
        class="primary"
        class:is-loading={loading || syncing}
        aria-busy={loading || syncing ? "true" : "false"}
        on:click={() => refreshUsage()}
        disabled={loading}
      >
        {loading || syncing ? "同步中" : "刷新"}
      </button>
    </section>
  </section>

  {#if metrics || initialLoading}
    <section class="metric-grid" class:loading-skeleton={initialLoading}>
      {#if metrics}
        <article>
          <span>当前筛选总 Token</span>
          <strong class="blue">{formatToken(metrics.totalTokens)}</strong>
        </article>
        <article>
          <span>输入 Token</span>
          <strong class="blue">{formatToken(metrics.inputTokens)}</strong>
        </article>
        <article>
          <span>缓存输入</span>
          <strong class="green">{formatToken(metrics.cachedInputTokens)}</strong>
        </article>
        <article>
          <span>非缓存输入</span>
          <strong class="orange">{formatToken(metrics.nonCachedInputTokens)}</strong>
        </article>
        <article>
          <span>输出 Token</span>
          <strong class="blue">{formatToken(metrics.outputTokens)}</strong>
        </article>
        <article>
          <span>推理输出</span>
          <strong class="blue">{formatToken(metrics.reasoningOutputTokens)}</strong>
        </article>
        <article>
          <span>TokenCount</span>
          <strong>{formatCount(metrics.tokenEventCount)}</strong>
        </article>
        <article>
          <span>超高行</span>
          <strong class="red">{formatCount(metrics.abnormalCount)}</strong>
        </article>
      {:else}
        {#each metricSkeletonLabels as label}
          <article>
            <span>{label}</span>
            <strong aria-hidden="true"></strong>
          </article>
        {/each}
      {/if}
    </section>
  {/if}

  {#if activePage === "detail" || activePage === "monitor"}
    {@const detailTableRows = activePage === "monitor" ? monitorRows : visibleDetailRows}
    {@const detailTotalRows = activePage === "monitor" ? monitorRows.length : data?.detailRows.length ?? 0}
    {@const isMonitorPage = activePage === "monitor"}
    <section class="panel detail-panel page-enter">
      <div class="panel-title detail-title">
        <div class="detail-controls-row">
          {#if isMonitorPage}
            <div class="detail-note">仅展示本次启动后新增或变动会话中的用户输入。</div>
          {:else}
            <div class="level-legend">
              {#each detailLevelControls as control}
                <button
                  type="button"
                  class:active={detailExpandLevel === control.level}
                  on:click={() => expandDetailToLevel(control.level)}
                  disabled={!data}
                >
                  <i class={`dot level-${control.level}`}>{control.icon}</i>
                  <strong>{control.label}</strong>
                  <span>{control.english}</span>
                </button>
              {/each}
            </div>
          {/if}
          <div class="detail-tools">
            <span class="row-count">{formatCount(detailTableRows.length)} / {formatCount(detailTotalRows)} 行</span>
            {#if !isMonitorPage}
              <button
                type="button"
                class:active={showOnlyAnomalies}
                on:click={toggleOnlyAnomalies}
                disabled={!data}
              >
                仅过高
              </button>
              <button on:click={exportDetail} disabled={!data}>导出明细</button>
            {/if}
          </div>
        </div>
      </div>

      <div class:monitor={isMonitorPage} class="table-wrap detail" bind:this={detailTableWrap}>
        <table>
          <colgroup>
            <col class="col-node" />
            {#if isMonitorPage}
              <col class="col-project-session" />
            {/if}
            <col class="col-last-time" />
            {#if !isMonitorPage}
              <col class="col-start-time" />
            {/if}
            <col class="col-status" />
            <col class="col-token" />
            <col class="col-token" />
            <col class="col-token" />
            <col class="col-token" />
            <col class="col-token" />
            <col class="col-token" />
          </colgroup>
          <thead>
            <tr>
              <th>
                {#if isMonitorPage}
                  用户输入
                {:else}
                  <button
                    type="button"
                    class="sortable-header"
                    class:sorted={detailSort.key === "node"}
                    on:click={() => updateDetailSort("node")}
                  >
                    节点 <span>{sortIndicator(detailSort.key === "node", detailSort.direction)}</span>
                  </button>
                {/if}
              </th>
              {#if isMonitorPage}
                <th>
                  <button
                    type="button"
                    class="sortable-header"
                    class:sorted={monitorSort.key === "projectSession"}
                    on:click={() => updateMonitorSort("projectSession")}
                  >
                    项目 / 会话 <span>{sortIndicator(monitorSort.key === "projectSession", monitorSort.direction)}</span>
                  </button>
                </th>
              {/if}
              <th>
                <button
                  type="button"
                  class="sortable-header"
                  class:sorted={isMonitorPage ? monitorSort.key === "lastTime" : detailSort.key === "lastTime"}
                  on:click={() => isMonitorPage ? updateMonitorSort("lastTime") : updateDetailSort("lastTime")}
                >
                  最后更新时间
                  <span>{sortIndicator(isMonitorPage ? monitorSort.key === "lastTime" : detailSort.key === "lastTime", isMonitorPage ? monitorSort.direction : detailSort.direction)}</span>
                </button>
              </th>
              {#if !isMonitorPage}
                <th>
                  <button
                    type="button"
                    class="sortable-header"
                    class:sorted={detailSort.key === "startTime"}
                    on:click={() => updateDetailSort("startTime")}
                  >
                    开始时间 <span>{sortIndicator(detailSort.key === "startTime", detailSort.direction)}</span>
                  </button>
                </th>
              {/if}
              <th>状态</th>
              <th>
                <button
                  type="button"
                  class="sortable-header"
                  class:sorted={isMonitorPage ? monitorSort.key === "totalTokens" : detailSort.key === "totalTokens"}
                  on:click={() => isMonitorPage ? updateMonitorSort("totalTokens") : updateDetailSort("totalTokens")}
                >
                  总计
                  <span>{sortIndicator(isMonitorPage ? monitorSort.key === "totalTokens" : detailSort.key === "totalTokens", isMonitorPage ? monitorSort.direction : detailSort.direction)}</span>
                </button>
              </th>
              <th>
                <span class="table-header-with-help">
                  输入
                  <button
                    type="button"
                    class="header-help"
                    aria-label="输入说明"
                    on:mouseenter={(event) => showTooltip(event, "输入 = 缓存输入 + 非缓存输入")}
                    on:mouseleave={scheduleHideTooltip}
                    on:focus={(event) => showTooltip(event, "输入 = 缓存输入 + 非缓存输入")}
                    on:blur={scheduleHideTooltip}
                  >?</button>
                </span>
              </th>
              <th>缓存输入</th>
              <th>非缓存输入</th>
              <th>
                <span class="table-header-with-help">
                  输出
                  <button
                    type="button"
                    class="header-help"
                    aria-label="输出说明"
                    on:mouseenter={(event) => showTooltip(event, "输出 = 推理输出 + 非推理输出")}
                    on:mouseleave={scheduleHideTooltip}
                    on:focus={(event) => showTooltip(event, "输出 = 推理输出 + 非推理输出")}
                    on:blur={scheduleHideTooltip}
                  >?</button>
                </span>
              </th>
              <th>推理输出</th>
            </tr>
          </thead>
          <tbody>
            {#if data && detailTableRows.length > 0}
              {#each detailTableRows as row}
              <tr>
                <td class="node" style={`--indent:${isMonitorPage ? 0 : row.level * 18}px`}>
                  {#if isMonitorPage}
                    <button
                      type="button"
                      class="node-label node-tooltip-trigger monitor-input-label"
                      on:mouseenter={(event) => showTooltip(event, detailTooltipText(row))}
                      on:mouseleave={scheduleHideTooltip}
                      on:focus={(event) => showTooltip(event, detailTooltipText(row))}
                      on:blur={scheduleHideTooltip}
                    >
                      {detailNodeDisplayText(row)}
                    </button>
                  {:else}
                    {#if row.hasChildren}
                      <button
                        class="tree-toggle"
                        aria-label={expandedDetailRows.has(row.rowKey) ? "折叠" : "展开"}
                        on:click={() => toggleDetailRow(row.rowKey)}
                      >
                        {expandedDetailRows.has(row.rowKey) ? "▾" : "▸"}
                      </button>
                    {:else}
                      <span class="tree-spacer"></span>
                    {/if}
                    <button
                      type="button"
                      class="node-label node-tooltip-trigger"
                      class:expandable={row.hasChildren}
                      aria-expanded={row.hasChildren ? expandedDetailRows.has(row.rowKey) : undefined}
                      on:click={() => row.hasChildren && toggleDetailRow(row.rowKey)}
                      on:mouseenter={(event) => showTooltip(event, detailTooltipText(row))}
                      on:mouseleave={scheduleHideTooltip}
                      on:focus={(event) => showTooltip(event, detailTooltipText(row))}
                      on:blur={scheduleHideTooltip}
                    >
                      <span class={`dot level-${row.level}`}>{levelLabel(row.level)}</span>
                      <strong class="node-type">{detailKindLabel(row.kind)}：</strong>{detailNodeDisplayText(row)}
                    </button>
                  {/if}
                  </td>
                {#if isMonitorPage}
                  <td class="project-session" title={detailProjectSessionText(row)}>{detailProjectSessionText(row)}</td>
                {/if}
                <td>
                  <span class="time-cell" class:updated={updatedDetailRows.has(row.rowKey)}>
                    {formatDetailLastTime(row)}
                  </span>
                </td>
                {#if !isMonitorPage}
                  <td>{formatDetailStartTime(row)}</td>
                {/if}
                <td><span class={`status ${statusClass(row.status)}`} title={row.statusReason}>{row.status}</span></td>
                <td class={`total ${totalToneClass(row.status)}`}>{formatToken(row.totalTokens)}</td>
                <td>{formatToken(row.inputTokens)}</td>
                <td>{formatToken(row.cachedInputTokens)}</td>
                <td>{formatToken(row.nonCachedInputTokens)}</td>
                <td>{formatToken(row.outputTokens)}</td>
                <td>{formatToken(row.reasoningOutputTokens)}</td>
              </tr>
              {/each}
            {:else}
              <tr>
                <td class="empty-row" colspan="10">
                  {loading
                    ? "正在读取本机 Codex 日志..."
                    : isMonitorPage
                      ? "本次启动后暂无新增或变动会话"
                      : "当前筛选条件下没有明细记录"}
                </td>
              </tr>
            {/if}
          </tbody>
        </table>
      </div>
    </section>
  {:else}
    <section class="stats-layout page-enter" class:initial-loading={initialLoading}>
      <section class="trend-card month-trend-panel">
        <h3>近6月-趋势</h3>
        {#if data && monthlyTrendBuckets.length > 0}
          <div class="vertical-chart">
            {#each monthlyTrendBuckets as bucket}
              <div class="chart-column" title={bucket.inRange ? `${bucket.label} ${formatToken(bucket.totalTokens)}` : "不在查询范围内"}>
                <b>{rangedTrendValueText(bucket, bucket.inRange)}</b>
                <div class="chart-track">
                  <i class={bucket.inRange ? trendClass(bucket, monthlyPeak) : ""} style={`height:${bucket.inRange ? trendHeight(bucket, monthlyPeak) : "0%"}`}></i>
                </div>
                <span>{bucket.displayLabel}</span>
              </div>
            {/each}
          </div>
        {:else if initialLoading}
          <div class="vertical-chart skeleton-chart">
            {#each Array.from({ length: 6 }) as _, index}
              <div class="chart-column">
                <b aria-hidden="true"></b>
                <div class="chart-track">
                  <i style={`height:${index >= 4 ? "82%" : "2%"}`}></i>
                </div>
                <span aria-hidden="true"></span>
              </div>
            {/each}
          </div>
        {:else}
          <div class="empty-block">暂无月趋势数据</div>
        {/if}
      </section>

      <section class="trend-card daily-trend-panel">
        <h3>近2周-趋势</h3>
        {#if data}
          <div class="vertical-chart daily-chart">
            {#each dailyTrendBuckets as bucket}
              <div class="chart-column" title={bucket.inRange ? `${bucket.label} ${formatToken(bucket.totalTokens)}` : "不在查询范围内"}>
                <b>{rangedTrendValueText(bucket, bucket.inRange, true)}</b>
                <div class="chart-track">
                  <i class={bucket.inRange ? trendClass(bucket, dailyPeak) : ""} style={`height:${bucket.inRange ? trendHeight(bucket, dailyPeak) : "0%"}`}></i>
                </div>
                <span>{bucket.displayLabel}</span>
              </div>
            {/each}
          </div>
        {:else if initialLoading}
          <div class="vertical-chart daily-chart skeleton-chart">
            {#each Array.from({ length: 14 }) as _, index}
              <div class="chart-column">
                <b aria-hidden="true"></b>
                <div class="chart-track">
                  <i style={`height:${index < 8 ? "2%" : "64%"}`}></i>
                </div>
                <span aria-hidden="true"></span>
              </div>
            {/each}
          </div>
        {:else}
          <div class="empty-block">暂无日趋势数据</div>
        {/if}
      </section>

      <section class="panel top-list-panel project-rank-panel">
        <div class="panel-title compact">
          <div>
            <h2>项目排行</h2>
          </div>
        </div>
        <div class="top-bars">
          {#if data && data.topProjects.length > 0}
            {#each data.topProjects.slice(0, 5) as project, index}
              <div class={`top-bar-row ${rankToneClass(project.totalTokens, topProjectPeak)}`}>
                <em>{index + 1}</em>
                <span>{project.projectName}</span>
                <div class="top-bar-track">
                  <i style={`width:${barWidth(project.totalTokens, topProjectPeak)}`}></i>
                </div>
                <b>{formatToken(project.totalTokens)}</b>
              </div>
            {/each}
          {:else if initialLoading}
            {#each Array.from({ length: 5 }) as _, index}
              <div class="top-bar-row skeleton-row">
                <em aria-hidden="true"></em>
                <span aria-hidden="true"></span>
                <div class="top-bar-track">
                  <i style={`width:${index === 0 ? "100%" : `${Math.max(8, 54 - index * 8)}%`}`}></i>
                </div>
                <b aria-hidden="true"></b>
              </div>
            {/each}
          {:else}
            <div class="top-empty">暂无项目数据</div>
          {/if}
        </div>
      </section>

      <section class="panel composition-panel composition-rank-panel">
        <div class="panel-title compact">
          <div>
            <h2>Token 构成</h2>
          </div>
        </div>
        <div class="composition">
          {#if metrics}
            <div class="composition-rings">
              <div class="composition-ring-item tone-medium">
                <div class="single-ring" style={`--ring:${totalRingStyle(metrics)}`}>
                  <div class="single-ring-center">
                    <span>总计</span>
                    <b>{formatToken(metrics.totalTokens)}</b>
                  </div>
                </div>
                <div class="ring-legend">
                  <div class="tone-medium"><i class="blue-bg"></i><span>输入：<b>{percentOf(metrics.inputTokens, metrics.totalTokens)}</b></span></div>
                  <div class="tone-cyan"><i class="cyan-bg"></i><span>输出：<b>{percentOf(outputTotal(metrics), metrics.totalTokens)}</b></span></div>
                </div>
              </div>

              <div class="composition-ring-item tone-low">
                <div class="ring-legend ring-legend-above">
                  <div class="tone-low"><i class="green-bg"></i><span>缓存输入：<b>{percentOf(metrics.cachedInputTokens, metrics.inputTokens)}</b></span></div>
                  <div class="tone-high"><i class="orange-bg"></i><span>非缓存输入：<b>{percentOf(metrics.nonCachedInputTokens, metrics.inputTokens)}</b></span></div>
                </div>
                <div class="single-ring" style={`--ring:${inputRingStyle(metrics)}`}>
                  <div class="single-ring-center">
                    <span>输入</span>
                    <b>{formatToken(metrics.inputTokens)}</b>
                  </div>
                </div>
              </div>

              <div class="composition-ring-item tone-medium">
                <div class="single-ring" style={`--ring:${outputRingStyle(metrics)}`}>
                  <div class="single-ring-center">
                    <span>输出</span>
                    <b>{formatToken(outputTotal(metrics))}</b>
                  </div>
                </div>
                <div class="ring-legend">
                  <div class="tone-medium"><i class="blue-bg"></i><span>推理输出：<b>{percentOf(metrics.reasoningOutputTokens, outputTotal(metrics))}</b></span></div>
                  <div class="tone-high"><i class="orange-bg"></i><span>非推理输出：<b>{percentOf(metrics.outputTokens, outputTotal(metrics))}</b></span></div>
                </div>
              </div>
            </div>
          {:else if initialLoading}
            <div class="composition-rings composition-skeleton">
              <div class="composition-ring-item">
                <div class="single-ring" aria-hidden="true"></div>
                <div class="ring-legend">
                  <div><i></i><span></span></div>
                  <div><i></i><span></span></div>
                </div>
              </div>
              <div class="composition-ring-item">
                <div class="single-ring" aria-hidden="true"></div>
                <div class="ring-legend">
                  <div><i></i><span></span></div>
                  <div><i></i><span></span></div>
                </div>
              </div>
              <div class="composition-ring-item">
                <div class="single-ring" aria-hidden="true"></div>
                <div class="ring-legend">
                  <div><i></i><span></span></div>
                  <div><i></i><span></span></div>
                </div>
              </div>
            </div>
          {:else}
            <div class="empty-block">{loading ? "正在计算构成..." : "暂无构成数据"}</div>
          {/if}
        </div>
      </section>

      <section class="panel heat-panel hourly-rank-panel">
        <div class="panel-title compact">
          <div>
            <h2>近2周-分时</h2>
          </div>
        </div>
        <div class="heatmap">
          <div class="hour-labels">
            {#each [0, 4, 8, 12, 16, 20, 24] as hour}
              <span style={`top:${(hour / 24) * 100}%`}>{hour}点</span>
            {/each}
          </div>
          <div
            class={`day-grid${hourlyDays.length > 7 ? " compact" : ""}`}
            style={`grid-template-columns: repeat(${Math.max(hourlyDays.length, 1)}, minmax(0, 1fr));`}
          >
            {#if hourlyDays.length > 0}
              {#each hourlyDays as day}
                <div class="day-column" class:outside-range={!day.inRange}>
                  <span>{day.inRange ? dayLabel(day.date) : "-"}</span>
                  {#each Array.from({ length: 24 }, (_, index) => index) as hour}
                    {@const bucket = day.inRange ? hourlyMap.get(`${day.date}|${hour}`) : undefined}
                    <div
                      class={bucketClass(bucket)}
                      title={day.inRange ? `${day.date} ${hour}:00 ${formatToken(bucket?.totalTokens ?? 0)}` : ""}
                    ></div>
                  {/each}
                </div>
              {/each}
            {:else}
              <div class="heat-empty">{loading ? "正在生成小时分布..." : "暂无小时数据"}</div>
            {/if}
          </div>
        </div>
      </section>

      <section class="panel top-list-panel session-rank-panel">
        <div class="panel-title compact">
          <div>
            <h2>会话排行</h2>
          </div>
        </div>
        <div class="top-bars">
          {#if data && data.topSessions.length > 0}
            {#each data.topSessions.slice(0, 5) as session, index}
              <div class={`top-bar-row session-top-row ${rankToneClass(session.totalTokens, topSessionPeak)}`} title={`${session.sessionName}\n项目：${session.projectName}\nSession ID：${session.sessionId}`}>
                <em>{index + 1}</em>
                <span class="top-project-name">{session.projectName}</span>
                <span class="top-session-name">{session.sessionName}</span>
                <div class="top-bar-track">
                  <i style={`width:${barWidth(session.totalTokens, topSessionPeak)}`}></i>
                </div>
                <b>{formatToken(session.totalTokens)}</b>
              </div>
            {/each}
          {:else if initialLoading}
            {#each Array.from({ length: 5 }) as _, index}
              <div class="top-bar-row skeleton-row">
                <em aria-hidden="true"></em>
                <span aria-hidden="true"></span>
                <div class="top-bar-track">
                  <i style={`width:${index === 0 ? "100%" : `${Math.max(8, 54 - index * 8)}%`}`}></i>
                </div>
                <b aria-hidden="true"></b>
              </div>
            {/each}
          {:else}
            <div class="top-empty">暂无会话数据</div>
          {/if}
        </div>
      </section>
    </section>
  {/if}

  {#if tooltip.visible}
    <div
      class={`floating-tooltip ${tooltip.placement}`}
      style={`left:${tooltip.x}px; top:${tooltip.y}px;`}
      role="tooltip"
      on:mouseenter={cancelTooltipHide}
      on:mouseleave={scheduleHideTooltip}
    >{tooltip.text}</div>
  {/if}

  <footer class="status-line">
    <button
      type="button"
      class="status-update-button"
      class:primary={updateButtonStatus === "ready" || updateButtonStatus === "installing"}
      class:latest={updateButtonStatus === "latest"}
      class:failed={updateButtonStatus === "failed"}
      class:progress={updateButtonStatus === "checking" || updateButtonStatus === "downloading" || updateButtonStatus === "installing"}
      on:click={handleUpdateButton}
      disabled={updateButtonDisabled()}
      title={updateResultMessage || updateButtonLabel()}
      aria-live="polite"
    >
      {updateButtonLabel()}
    </button>
    {#if syncStatusMessage}
      <span role="status" aria-live="polite">{syncStatusMessage}</span>
    {:else if data?.performanceTimings?.totalMs}
      <span>查询耗时：{data.performanceTimings.totalMs} ms</span>
    {/if}
    {#if appVersion}
      <span>版本：{appVersion}</span>
    {/if}
    <span>账本：{formatCount(data?.scanState.ledgerTokenEvents ?? 0)} 条</span>
    <span>本次新增：{formatCount(data?.scanState.lastRunNewTokenEvents ?? 0)} 条</span>
    <span>扫描文件：{formatCount(data?.scanState.lastRunFilesScanned ?? 0)} 个</span>
    <span>解析失败：{formatCount(data?.scanState.lastRunParseErrors ?? 0)} 行</span>
    <span>截止：{timeText(data?.scanState.lastCutoffUtc)}</span>
  </footer>
</main>
