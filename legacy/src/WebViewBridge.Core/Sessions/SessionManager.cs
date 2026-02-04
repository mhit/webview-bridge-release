using System.Collections.Concurrent;
using Microsoft.Extensions.Logging;

namespace WebViewBridge.Core.Sessions;

public class SessionManager : IDisposable
{
    private readonly ConcurrentDictionary<string, Session> _sessions = new();
    private readonly ILogger<SessionManager> _logger;
    private readonly int _maxSessions;
    private readonly TimeSpan _sessionTimeout;
    private readonly Timer _cleanupTimer;
    private bool _disposed;

    public SessionManager(ILogger<SessionManager> logger, int maxSessions = 5, int sessionTimeoutMinutes = 30)
    {
        _logger = logger;
        _maxSessions = maxSessions;
        _sessionTimeout = TimeSpan.FromMinutes(sessionTimeoutMinutes);
        _cleanupTimer = new Timer(CleanupExpiredSessions, null, TimeSpan.FromMinutes(1), TimeSpan.FromMinutes(1));
    }

    public async Task<Session> CreateSessionAsync(SessionOptions options)
    {
        if (_sessions.Count >= _maxSessions)
        {
            _logger.LogWarning("Session limit reached ({Max}), cleaning up expired sessions", _maxSessions);
            CleanupExpiredSessions(null);
            
            if (_sessions.Count >= _maxSessions)
            {
                throw new InvalidOperationException($"Maximum session limit ({_maxSessions}) reached");
            }
        }

        var id = $"sess_{Guid.NewGuid():N}"[..16];
        var session = new Session(id, options);

        if (!_sessions.TryAdd(id, session))
        {
            throw new InvalidOperationException("Failed to create session");
        }

        _logger.LogInformation("Session created: {SessionId}, Profile: {Profile}", id, options.Profile);
        return session;
    }

    public Session? GetSession(string sessionId)
    {
        if (_sessions.TryGetValue(sessionId, out var session))
        {
            if (session.Status != SessionStatus.Disposed)
            {
                session.Touch();
                return session;
            }
        }
        return null;
    }

    public bool TryRemoveSession(string sessionId, out Session? session)
    {
        if (_sessions.TryRemove(sessionId, out session))
        {
            session.Dispose();
            _logger.LogInformation("Session removed: {SessionId}", sessionId);
            return true;
        }
        session = null;
        return false;
    }

    public IReadOnlyCollection<Session> GetAllSessions()
    {
        return _sessions.Values.ToList().AsReadOnly();
    }

    public int ActiveSessionCount => _sessions.Count(s => s.Value.Status != SessionStatus.Disposed);

    private void CleanupExpiredSessions(object? state)
    {
        var expiredSessions = _sessions
            .Where(kv => kv.Value.IsExpired(_sessionTimeout) || kv.Value.Status == SessionStatus.Disposed)
            .Select(kv => kv.Key)
            .ToList();

        foreach (var sessionId in expiredSessions)
        {
            if (_sessions.TryRemove(sessionId, out var session))
            {
                session.Dispose();
                _logger.LogInformation("Expired session cleaned up: {SessionId}", sessionId);
            }
        }

        if (expiredSessions.Count > 0)
        {
            _logger.LogDebug("Cleaned up {Count} expired sessions", expiredSessions.Count);
        }
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;

        _cleanupTimer.Dispose();

        foreach (var session in _sessions.Values)
        {
            session.Dispose();
        }
        _sessions.Clear();

        _logger.LogInformation("SessionManager disposed");
    }
}
