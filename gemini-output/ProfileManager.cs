using System.Text.Json.Serialization;

namespace WebViewBridge.Core.Profiles
{
    /// <summary>
    /// Represents a WebView2 profile configuration.
    /// </summary>
    public class Profile
    {
        /// <summary>
        /// Gets or sets the name of the profile. This is used for the UserDataFolder.
        /// </summary>
        public string Name { get; set; } = "Default";

        /// <summary>
        /// Gets or sets a custom user agent string for the profile.
        /// If null or empty, the default WebView2 user agent will be used.
        /// </summary>
        public string? UserAgent { get; set; }

        /// <summary>
        /// Gets or sets the proxy server settings.
        /// </summary>
        public ProxySettings? Proxy { get; set; }

        /// <summary>
        /// Gets the full path to the user data folder for this profile.
        /// This property is not serialized to JSON.
        /// </summary>
        [JsonIgnore]
        public string UserDataFolder { get; internal set; } = string.Empty;
    }

    /// <summary>
    /// Represents proxy settings for a profile.
    /// </summary>
    public class ProxySettings
    {
        /// <summary>
        /// Gets or sets the proxy server address (e.g., "127.0.0.1:8080").
        /// </summary>
        public string? Server { get; set; }

        /// <summary>
        /// Gets or sets a bypass list for the proxy, separated by semicolons.
        /// </summary>
        public string? BypassList { get; set; }
    }
}
```
```csharp
using System;
using System.IO;
using System.Text.Json;
using System.Threading.Tasks;
using Microsoft.Extensions.Logging;

namespace WebViewBridge.Core.Profiles
{
    /// <summary>
    /// Manages WebView2 user profiles, including creation, loading, and persistence.
    /// </summary>
    public class ProfileManager
    {
        private readonly string _baseProfilesPath;
        private readonly ILogger<ProfileManager> _logger;
        private readonly JsonSerializerOptions _jsonSerializerOptions;

        /// <summary>
        /// The name of the default profile.
        /// </summary>
        public const string DefaultProfileName = "Default";

        /// <summary>
        /// Initializes a new instance of the <see cref="ProfileManager"/> class.
        /// </summary>
        /// <param name="logger">The logger instance for logging messages.</param>
        public ProfileManager(ILogger<ProfileManager> logger)
        {
            _logger = logger;
            _baseProfilesPath = Path.Combine(
                Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
                "WebViewBridge",
                "profiles"
            );
            _jsonSerializerOptions = new JsonSerializerOptions
            {
                WriteIndented = true,
                DefaultIgnoreCondition = System.Text.Json.Serialization.JsonIgnoreCondition.WhenWritingNull
            };

            _logger.LogInformation("ProfileManager initialized. Base path: {BasePath}", _baseProfilesPath);
        }

        /// <summary>
        /// Gets the settings file path for a given profile name.
        /// </summary>
        /// <param name="profileName">The name of the profile.</param>
        /// <returns>The full path to the profile's settings.json file.</returns>
        private string GetSettingsFilePath(string profileName) =>
            Path.Combine(_baseProfilesPath, profileName, "settings.json");

        /// <summary>
        /// Gets the user data folder path for a given profile name.
        /// </summary>
        /// <param name="profileName">The name of the profile.</param>
        /// <returns>The full path to the profile's user data folder.</returns>
        private string GetUserDataFolder(string profileName) =>
            Path.Combine(_baseProfilesPath, profileName);

        /// <summary>
        /// Retrieves a profile by name. If the profile does not exist, it creates a new one with default settings.
        /// </summary>
        /// <param name="name">The name of the profile. Must be a valid directory name.</param>
        /// <returns>A task that represents the asynchronous operation. The task result contains the loaded or newly created profile.</returns>
        public async Task<Profile> GetProfileAsync(string name)
        {
            if (string.IsNullOrWhiteSpace(name) || name.IndexOfAny(Path.GetInvalidFileNameChars()) >= 0)
            {
                _logger.LogError("Invalid profile name provided: {ProfileName}", name);
                throw new ArgumentException("Profile name contains invalid characters.", nameof(name));
            }

            var settingsFilePath = GetSettingsFilePath(name);
            var userDataFolder = GetUserDataFolder(name);

            if (File.Exists(settingsFilePath))
            {
                try
                {
                    _logger.LogDebug("Loading existing profile: {ProfileName}", name);
                    var json = await File.ReadAllTextAsync(settingsFilePath);
                    var profile = JsonSerializer.Deserialize<Profile>(json, _jsonSerializerOptions);

                    if (profile != null)
                    {
                        profile.UserDataFolder = userDataFolder;
                        return profile;
                    }

                    _logger.LogWarning("Failed to deserialize profile '{ProfileName}'. A new default profile will be created.", name);
                }
                catch (Exception ex)
                {
                    _logger.LogError(ex, "Error reading or parsing profile settings for '{ProfileName}'. A new default profile will be created.", name);
                }
            }

            _logger.LogInformation("Creating new profile: {ProfileName}", name);
            var newProfile = new Profile
            {
                Name = name,
                UserDataFolder = userDataFolder
            };
            await SaveProfileAsync(newProfile);
            return newProfile;
        }

        /// <summary>
        /// Saves a profile's settings to its corresponding settings.json file.
        /// </summary>
        /// <param name="profile">The profile to save.</param>
        /// <returns>A task that represents the asynchronous operation.</returns>
        public async Task SaveProfileAsync(Profile profile)
        {
            if (profile == null)
            {
                throw new ArgumentNullException(nameof(profile));
            }

            var settingsFilePath = GetSettingsFilePath(profile.Name);
            var profileDirectory = Path.GetDirectoryName(settingsFilePath);

            try
            {
                if (profileDirectory != null && !Directory.Exists(profileDirectory))
                {
                    Directory.CreateDirectory(profileDirectory);
                    _logger.LogInformation("Created profile directory: {Directory}", profileDirectory);
                }

                var json = JsonSerializer.Serialize(profile, _jsonSerializerOptions);
                await File.WriteAllTextAsync(settingsFilePath, json);
                _logger.LogDebug("Successfully saved profile: {ProfileName}", profile.Name);
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Failed to save profile: {ProfileName}", profile.Name);
                throw; // Re-throw to let the caller know something went wrong.
            }
        }

        /// <summary>
        /// Gets the default profile.
        /// </summary>
        /// <returns>A task that represents the asynchronous operation. The task result contains the default profile.</returns>
        public Task<Profile> GetDefaultProfileAsync()
        {
            return GetProfileAsync(DefaultProfileName);
        }
    }
}
