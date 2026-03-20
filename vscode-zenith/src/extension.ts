/**
 * vscode-zenith — VS Code extension for the Zenith workflow runner.
 *
 * Features:
 *  - Run zenith / zenith run <job> from the command palette or editor title bar
 *  - Streams output to a dedicated Output Channel
 *  - Open the web dashboard in a WebviewPanel (embedded) or browser tab
 *  - Status bar item shows active run status
 *  - YAML schema validation via yamlValidation contribution point
 */

import * as vscode from 'vscode';
import * as cp from 'child_process';
import * as path from 'path';

// ─── State ────────────────────────────────────────────────────────────────────

let outputChannel: vscode.OutputChannel;
let statusBarItem: vscode.StatusBarItem;
let activeProcess: cp.ChildProcess | null = null;

// ─── Activation ───────────────────────────────────────────────────────────────

export function activate(context: vscode.ExtensionContext): void {
    outputChannel = vscode.window.createOutputChannel('Zenith');
    statusBarItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 100);
    statusBarItem.command = 'zenith.showOutput';
    statusBarItem.text = '$(zap) Zenith';
    statusBarItem.tooltip = 'Zenith workflow runner';
    statusBarItem.show();

    context.subscriptions.push(outputChannel, statusBarItem);

    // Register commands
    context.subscriptions.push(
        vscode.commands.registerCommand('zenith.run',           () => runWorkflow(null)),
        vscode.commands.registerCommand('zenith.runJob',        () => runWithJobPicker()),
        vscode.commands.registerCommand('zenith.openDashboard', () => openDashboard(context)),
        vscode.commands.registerCommand('zenith.openTui',       () => openTui()),
        vscode.commands.registerCommand('zenith.cacheClean',    () => runZenith(['cache', 'clean'])),
        vscode.commands.registerCommand('zenith.showOutput',    () => outputChannel.show()),
    );

    // Watch .zenith.yml for changes — refresh diagnostics
    const watcher = vscode.workspace.createFileSystemWatcher('**/.zenith.yml');
    watcher.onDidChange(() => validateConfig());
    watcher.onDidCreate(() => validateConfig());
    context.subscriptions.push(watcher);

    validateConfig();
}

export function deactivate(): void {
    activeProcess?.kill();
}

// ─── Core runner ─────────────────────────────────────────────────────────────

function zenithBin(): string {
    const cfg = vscode.workspace.getConfiguration('zenith');
    return cfg.get<string>('binaryPath') ?? 'zenith';
}

function workspaceRoot(): string | undefined {
    return vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
}

function runZenith(args: string[], cwd?: string): cp.ChildProcess {
    const bin  = zenithBin();
    const root = cwd ?? workspaceRoot() ?? process.cwd();

    outputChannel.appendLine(`\n[zenith] ${bin} ${args.join(' ')}`);
    outputChannel.appendLine(`[zenith] cwd: ${root}`);
    outputChannel.appendLine('[zenith] ' + '─'.repeat(50));
    outputChannel.show(true);

    const proc = cp.spawn(bin, args, { cwd: root, shell: true });
    activeProcess = proc;

    proc.stdout?.on('data', (d: Buffer) => outputChannel.append(d.toString()));
    proc.stderr?.on('data', (d: Buffer) => outputChannel.append(d.toString()));

    proc.on('exit', (code) => {
        activeProcess = null;
        const ok = code === 0;
        outputChannel.appendLine(`\n[zenith] exited with code ${code}`);
        setStatus(ok ? 'success' : 'failed');
        if (ok) {
            vscode.window.showInformationMessage('Zenith: workflow completed successfully.');
        } else {
            vscode.window.showErrorMessage(`Zenith: workflow failed (exit ${code}).`);
        }
    });

    setStatus('running');
    return proc;
}

// ─── Commands ─────────────────────────────────────────────────────────────────

async function runWorkflow(job: string | null): Promise<void> {
    if (activeProcess) {
        const kill = await vscode.window.showWarningMessage(
            'A Zenith run is already in progress. Kill it and start a new one?',
            'Kill & Restart', 'Cancel',
        );
        if (kill !== 'Kill & Restart') { return; }
        activeProcess.kill();
    }

    const args = job ? ['run', '--job', job] : ['run'];
    runZenith(args);

    const cfg = vscode.workspace.getConfiguration('zenith');
    if (cfg.get<boolean>('autoOpenDashboard')) {
        // Give the server a moment to start, then open
        setTimeout(() => openDashboard(undefined), 1500);
    }
}

async function runWithJobPicker(): Promise<void> {
    const root = workspaceRoot();
    if (!root) {
        vscode.window.showErrorMessage('Open a workspace folder first.');
        return;
    }

    // Parse .zenith.yml to get job names
    const jobs = await getJobNames(root);
    if (!jobs.length) {
        vscode.window.showInformationMessage('No jobs found in .zenith.yml. Running default workflow.');
        return runWorkflow(null);
    }

    const choice = await vscode.window.showQuickPick(jobs, {
        placeHolder: 'Select a job to run',
        title: 'Zenith: Run Job',
    });
    if (choice) { await runWorkflow(choice); }
}

async function getJobNames(root: string): Promise<string[]> {
    const configPath = path.join(root, '.zenith.yml');
    try {
        const doc = await vscode.workspace.openTextDocument(configPath);
        const text = doc.getText();
        // Quick regex parse — good enough for job name discovery
        const matches = text.match(/^(\w[\w-]*):/gm) ?? [];
        return matches
            .map(m => m.replace(':', '').trim())
            .filter(n => !['on', 'jobs', 'env', 'name', 'steps', 'cache', 'strategy'].includes(n));
    } catch {
        return [];
    }
}

// ─── Dashboard WebView ────────────────────────────────────────────────────────

let dashboardPanel: vscode.WebviewPanel | undefined;

function openDashboard(context: vscode.ExtensionContext | undefined): void {
    const cfg  = vscode.workspace.getConfiguration('zenith');
    const port = cfg.get<number>('dashboardPort') ?? 7622;
    const url  = `http://localhost:${port}`;

    // Option 1: reuse existing WebView panel
    if (dashboardPanel) {
        dashboardPanel.reveal(vscode.ViewColumn.Beside);
        return;
    }

    if (!context) {
        // If no context (called from autoOpen), open in external browser
        vscode.env.openExternal(vscode.Uri.parse(url));
        return;
    }

    dashboardPanel = vscode.window.createWebviewPanel(
        'zenithDashboard',
        'Zenith Dashboard',
        vscode.ViewColumn.Beside,
        {
            enableScripts: true,
            retainContextWhenHidden: true,
        },
    );

    dashboardPanel.webview.html = getDashboardProxyHtml(url);

    dashboardPanel.onDidDispose(() => {
        dashboardPanel = undefined;
    }, null, context.subscriptions);
}

function getDashboardProxyHtml(url: string): string {
    return `<!DOCTYPE html>
<html>
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <style>
    body, html { margin: 0; padding: 0; height: 100%; overflow: hidden; }
    iframe { width: 100%; height: 100%; border: none; }
    .notice {
      position: fixed; bottom: 8px; right: 8px; background: #161b22;
      color: #8b949e; font-family: system-ui; font-size: 11px;
      padding: 4px 10px; border-radius: 4px; border: 1px solid #30363d;
    }
  </style>
</head>
<body>
  <iframe src="${url}" allow="scripts"></iframe>
  <div class="notice">
    Zenith dashboard — <a href="${url}" style="color:#58a6ff">open in browser</a>
    &nbsp;|&nbsp;make sure <code>zenith ui</code> is running
  </div>
</body>
</html>`;
}

// ─── TUI (integrated terminal) ────────────────────────────────────────────────

function openTui(): void {
    const term = vscode.window.createTerminal({
        name: 'Zenith TUI',
        cwd: workspaceRoot(),
    });
    term.sendText(`${zenithBin()} tui`);
    term.show();
}

// ─── Status bar ───────────────────────────────────────────────────────────────

function setStatus(state: 'running' | 'success' | 'failed'): void {
    switch (state) {
        case 'running':
            statusBarItem.text = '$(sync~spin) Zenith: running';
            statusBarItem.backgroundColor = undefined;
            break;
        case 'success':
            statusBarItem.text = '$(check) Zenith: success';
            statusBarItem.backgroundColor = undefined;
            break;
        case 'failed':
            statusBarItem.text = '$(error) Zenith: failed';
            statusBarItem.backgroundColor = new vscode.ThemeColor('statusBarItem.errorBackground');
            break;
    }
}

// ─── Config validation (diagnostics) ─────────────────────────────────────────

const diagnosticCollection = vscode.languages.createDiagnosticCollection('zenith');

async function validateConfig(): Promise<void> {
    diagnosticCollection.clear();
    const root = workspaceRoot();
    if (!root) { return; }

    const configPath = path.join(root, '.zenith.yml');
    let doc: vscode.TextDocument;
    try {
        doc = await vscode.workspace.openTextDocument(configPath);
    } catch {
        return;   // file doesn't exist — no diagnostics
    }

    const text = doc.getText();
    const diags: vscode.Diagnostic[] = [];

    // Warn if no 'jobs:' key is present
    if (!text.includes('jobs:')) {
        const range = new vscode.Range(0, 0, 0, 0);
        diags.push(new vscode.Diagnostic(
            range,
            "No 'jobs:' section found. Add a jobs block to define workflow steps.",
            vscode.DiagnosticSeverity.Warning,
        ));
    }

    diagnosticCollection.set(doc.uri, diags);
}
