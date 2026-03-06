export type AppLanguage = "en-US" | "es-ES" | "fr-FR" | "de-DE" | "pt-BR";

export type AppLanguageOption = {
  value: AppLanguage;
  englishLabel: string;
  nativeLabel: string;
};

export const APP_LANGUAGE_OPTIONS: AppLanguageOption[] = [
  {
    value: "en-US",
    englishLabel: "English (United States)",
    nativeLabel: "English (United States)",
  },
  {
    value: "es-ES",
    englishLabel: "Spanish (Spain)",
    nativeLabel: "Español (España)",
  },
  {
    value: "fr-FR",
    englishLabel: "French (France)",
    nativeLabel: "Français (France)",
  },
  {
    value: "de-DE",
    englishLabel: "German (Germany)",
    nativeLabel: "Deutsch (Deutschland)",
  },
  {
    value: "pt-BR",
    englishLabel: "Portuguese (Brazil)",
    nativeLabel: "Português (Brasil)",
  },
];

const EN_TRANSLATIONS = {
  "nav.home": "Home",
  "nav.discover": "Discover content",
  "nav.creator_studio": "Creator Studio",
  "nav.library": "Library",
  "nav.updates": "Updates available",
  "nav.skins": "Skins",
  "nav.create_instance": "Create new instance",
  "nav.dev": "Dev",
  "nav.account": "Account",
  "nav.settings": "Settings",
  "settings.title": "Settings",
  "settings.intro": "Appearance, account, and launcher behavior.",
  "settings.mode.basic": "Basic mode",
  "settings.mode.advanced": "Advanced mode",
  "settings.appearance.section_title": "Appearance",
  "settings.appearance.section_sub": "Tune the app look without changing layout behavior.",
  "settings.appearance.theme.title": "Theme",
  "settings.appearance.theme.sub": "Switch between dark and light.",
  "settings.appearance.theme.dark": "Dark",
  "settings.appearance.theme.light": "Light",
  "settings.appearance.accent.title": "Accent",
  "settings.appearance.accent.sub": "Pick an accent. Neutral stays subtle, colors are bolder.",
  "settings.appearance.accent_strength.title": "Accent strength",
  "settings.appearance.accent_strength.sub": "Adjust accent opacity and intensity from subtle to max.",
  "settings.appearance.motion.title": "Motion profile",
  "settings.appearance.motion.sub": "Choose how animated the interface should feel.",
  "settings.appearance.density.title": "UI density",
  "settings.appearance.density.sub": "Comfortable keeps more space, compact fits more on screen.",
  "settings.appearance.reset.title": "Reset UI settings",
  "settings.appearance.reset.sub":
    "Restore theme, accent, accent strength, motion profile, and density to defaults.",
  "settings.appearance.reset.button": "Reset appearance",
  "settings.language.section_title": "Language",
  "settings.language.section_sub": "Choose the language used by OpenJar Launcher.",
  "settings.language.preference.title": "App language",
  "settings.language.preference.sub": "Changes apply immediately and fall back to English for any untranslated copy.",
  "settings.language.preference.menu_prefix": "Language",
  "settings.language.warning":
    "This first localization pass covers navigation and the main Settings experience. Untranslated areas continue in English for now.",
  "settings.language.saving": "Saving language…",
  "settings.language.saved_notice": "App language updated to {language}.",
  "settings.launch.section_title": "Launch configuration",
  "settings.launch.section_sub": "Set default launcher behavior and Java runtime.",
  "settings.launch.method.title": "Default launch method",
  "settings.launch.method.sub": "Use native launcher or Prism launcher by default.",
  "settings.launch.method.native": "Native",
  "settings.launch.method.prism": "Prism",
  "settings.launch.java.title": "Java executable",
  "settings.launch.java.sub":
    "Absolute path to Java, or leave blank to use `java` from PATH. Minecraft 1.20.5+ needs Java 21+.",
  "settings.launch.java.browse": "Browse…",
  "settings.launch.java.detect": "Detect installed Java",
  "settings.launch.java.detecting": "Detecting…",
  "settings.launch.java.get_java_21": "Get Java 21",
  "settings.launch.java.selected": "Selected",
  "settings.launch.java.use": "Use",
  "settings.launch.oauth.title": "OAuth client ID override",
  "settings.launch.oauth.sub":
    "Client ID is a public identifier, not a secret API key. Leave blank to use the bundled default.",
  "settings.launch.oauth.placeholder": "Optional override client ID",
  "settings.launch.basic_hidden": "Advanced Java and OAuth overrides are hidden in Basic mode.",
  "settings.launch.switch_to_advanced": "Switch to Advanced",
  "settings.launch.save": "Save launcher settings",
  "settings.launch.saving": "Saving…",
  "settings.launch.saved_notice": "Launcher settings saved.",
  "settings.account.section_title": "Microsoft account",
  "settings.account.section_sub":
    "Connect the Microsoft account that owns Minecraft. You normally do not need to configure any client ID.",
  "settings.updates.section_title": "App updates",
  "settings.content.section_title": "Content and visuals",
  "settings.advanced.section_title": "Advanced settings",
} as const;

export type AppTranslationKey = keyof typeof EN_TRANSLATIONS;

type TranslationDictionary = Record<AppTranslationKey, string>;

const ES_TRANSLATIONS: TranslationDictionary = {
  "nav.home": "Inicio",
  "nav.discover": "Descubrir contenido",
  "nav.creator_studio": "Creator Studio",
  "nav.library": "Biblioteca",
  "nav.updates": "Actualizaciones disponibles",
  "nav.skins": "Skins",
  "nav.create_instance": "Crear instancia",
  "nav.dev": "Dev",
  "nav.account": "Cuenta",
  "nav.settings": "Ajustes",
  "settings.title": "Ajustes",
  "settings.intro": "Apariencia, cuenta y comportamiento del launcher.",
  "settings.mode.basic": "Modo básico",
  "settings.mode.advanced": "Modo avanzado",
  "settings.appearance.section_title": "Apariencia",
  "settings.appearance.section_sub": "Ajusta el aspecto de la app sin cambiar el diseño.",
  "settings.appearance.theme.title": "Tema",
  "settings.appearance.theme.sub": "Cambia entre oscuro y claro.",
  "settings.appearance.theme.dark": "Oscuro",
  "settings.appearance.theme.light": "Claro",
  "settings.appearance.accent.title": "Acento",
  "settings.appearance.accent.sub": "Elige un acento. Neutral se mantiene sutil y los colores son más intensos.",
  "settings.appearance.accent_strength.title": "Intensidad del acento",
  "settings.appearance.accent_strength.sub": "Ajusta la opacidad e intensidad del acento de sutil a máxima.",
  "settings.appearance.motion.title": "Perfil de movimiento",
  "settings.appearance.motion.sub": "Elige cuánto movimiento debe tener la interfaz.",
  "settings.appearance.density.title": "Densidad de la interfaz",
  "settings.appearance.density.sub": "Cómodo deja más espacio; compacto muestra más contenido.",
  "settings.appearance.reset.title": "Restablecer ajustes visuales",
  "settings.appearance.reset.sub":
    "Restaura tema, acento, intensidad del acento, perfil de movimiento y densidad a sus valores por defecto.",
  "settings.appearance.reset.button": "Restablecer apariencia",
  "settings.language.section_title": "Idioma",
  "settings.language.section_sub": "Elige el idioma que usa OpenJar Launcher.",
  "settings.language.preference.title": "Idioma de la app",
  "settings.language.preference.sub":
    "Los cambios se aplican de inmediato y usan inglés como respaldo para cualquier texto no traducido.",
  "settings.language.preference.menu_prefix": "Idioma",
  "settings.language.warning":
    "Esta primera fase de localización cubre la navegación y la experiencia principal de Ajustes. Las áreas no traducidas seguirán en inglés por ahora.",
  "settings.language.saving": "Guardando idioma…",
  "settings.language.saved_notice": "Idioma de la app actualizado a {language}.",
  "settings.launch.section_title": "Configuración de lanzamiento",
  "settings.launch.section_sub": "Define el comportamiento por defecto del launcher y el runtime de Java.",
  "settings.launch.method.title": "Método de lanzamiento por defecto",
  "settings.launch.method.sub": "Usa el launcher nativo o Prism por defecto.",
  "settings.launch.method.native": "Nativo",
  "settings.launch.method.prism": "Prism",
  "settings.launch.java.title": "Ejecutable de Java",
  "settings.launch.java.sub":
    "Ruta absoluta a Java, o déjalo vacío para usar `java` desde PATH. Minecraft 1.20.5+ necesita Java 21+.",
  "settings.launch.java.browse": "Buscar…",
  "settings.launch.java.detect": "Detectar Java instalado",
  "settings.launch.java.detecting": "Detectando…",
  "settings.launch.java.get_java_21": "Obtener Java 21",
  "settings.launch.java.selected": "Seleccionado",
  "settings.launch.java.use": "Usar",
  "settings.launch.oauth.title": "Sobrescritura del ID de cliente OAuth",
  "settings.launch.oauth.sub":
    "El ID de cliente es un identificador público, no una clave secreta. Déjalo vacío para usar el valor integrado.",
  "settings.launch.oauth.placeholder": "ID de cliente opcional",
  "settings.launch.basic_hidden": "Las opciones avanzadas de Java y OAuth están ocultas en el modo básico.",
  "settings.launch.switch_to_advanced": "Cambiar a avanzado",
  "settings.launch.save": "Guardar ajustes del launcher",
  "settings.launch.saving": "Guardando…",
  "settings.launch.saved_notice": "Ajustes del launcher guardados.",
  "settings.account.section_title": "Cuenta de Microsoft",
  "settings.account.section_sub":
    "Conecta la cuenta de Microsoft que posee Minecraft. Normalmente no necesitas configurar ningún ID de cliente.",
  "settings.updates.section_title": "Actualizaciones de la app",
  "settings.content.section_title": "Contenido y visuales",
  "settings.advanced.section_title": "Ajustes avanzados",
};

const FR_TRANSLATIONS: TranslationDictionary = {
  "nav.home": "Accueil",
  "nav.discover": "Découvrir du contenu",
  "nav.creator_studio": "Creator Studio",
  "nav.library": "Bibliothèque",
  "nav.updates": "Mises à jour disponibles",
  "nav.skins": "Skins",
  "nav.create_instance": "Créer une instance",
  "nav.dev": "Dev",
  "nav.account": "Compte",
  "nav.settings": "Paramètres",
  "settings.title": "Paramètres",
  "settings.intro": "Apparence, compte et comportement du launcher.",
  "settings.mode.basic": "Mode basique",
  "settings.mode.advanced": "Mode avancé",
  "settings.appearance.section_title": "Apparence",
  "settings.appearance.section_sub": "Ajustez l’apparence de l’application sans changer sa structure.",
  "settings.appearance.theme.title": "Thème",
  "settings.appearance.theme.sub": "Basculez entre sombre et clair.",
  "settings.appearance.theme.dark": "Sombre",
  "settings.appearance.theme.light": "Clair",
  "settings.appearance.accent.title": "Accent",
  "settings.appearance.accent.sub": "Choisissez une couleur d’accent. Neutral reste discret, les couleurs sont plus marquées.",
  "settings.appearance.accent_strength.title": "Intensité de l’accent",
  "settings.appearance.accent_strength.sub": "Ajustez l’opacité et l’intensité de l’accent de discret à maximal.",
  "settings.appearance.motion.title": "Profil d’animation",
  "settings.appearance.motion.sub": "Choisissez le niveau d’animation de l’interface.",
  "settings.appearance.density.title": "Densité de l’interface",
  "settings.appearance.density.sub": "Confortable laisse plus d’espace, compact affiche plus d’éléments.",
  "settings.appearance.reset.title": "Réinitialiser l’interface",
  "settings.appearance.reset.sub":
    "Restaurez le thème, l’accent, son intensité, le profil d’animation et la densité par défaut.",
  "settings.appearance.reset.button": "Réinitialiser l’apparence",
  "settings.language.section_title": "Langue",
  "settings.language.section_sub": "Choisissez la langue utilisée par OpenJar Launcher.",
  "settings.language.preference.title": "Langue de l’application",
  "settings.language.preference.sub":
    "Les changements s’appliquent immédiatement et retombent sur l’anglais pour tout texte non traduit.",
  "settings.language.preference.menu_prefix": "Langue",
  "settings.language.warning":
    "Cette première passe de localisation couvre la navigation et l’expérience principale des Paramètres. Les zones non traduites restent en anglais pour l’instant.",
  "settings.language.saving": "Enregistrement de la langue…",
  "settings.language.saved_notice": "Langue de l’application mise à jour : {language}.",
  "settings.launch.section_title": "Configuration de lancement",
  "settings.launch.section_sub": "Définissez le comportement par défaut du launcher et l’environnement Java.",
  "settings.launch.method.title": "Méthode de lancement par défaut",
  "settings.launch.method.sub": "Utilisez le launcher natif ou Prism par défaut.",
  "settings.launch.method.native": "Natif",
  "settings.launch.method.prism": "Prism",
  "settings.launch.java.title": "Exécutable Java",
  "settings.launch.java.sub":
    "Chemin absolu vers Java, ou laissez vide pour utiliser `java` depuis le PATH. Minecraft 1.20.5+ nécessite Java 21+.",
  "settings.launch.java.browse": "Parcourir…",
  "settings.launch.java.detect": "Détecter Java installé",
  "settings.launch.java.detecting": "Détection…",
  "settings.launch.java.get_java_21": "Obtenir Java 21",
  "settings.launch.java.selected": "Sélectionné",
  "settings.launch.java.use": "Utiliser",
  "settings.launch.oauth.title": "Surcharge de l’ID client OAuth",
  "settings.launch.oauth.sub":
    "L’ID client est un identifiant public, pas une clé secrète. Laissez vide pour utiliser la valeur intégrée.",
  "settings.launch.oauth.placeholder": "ID client facultatif",
  "settings.launch.basic_hidden": "Les réglages avancés Java et OAuth sont masqués en mode basique.",
  "settings.launch.switch_to_advanced": "Passer en avancé",
  "settings.launch.save": "Enregistrer les paramètres du launcher",
  "settings.launch.saving": "Enregistrement…",
  "settings.launch.saved_notice": "Paramètres du launcher enregistrés.",
  "settings.account.section_title": "Compte Microsoft",
  "settings.account.section_sub":
    "Connectez le compte Microsoft qui possède Minecraft. Vous n’avez normalement pas besoin de configurer d’ID client.",
  "settings.updates.section_title": "Mises à jour de l’application",
  "settings.content.section_title": "Contenu et visuels",
  "settings.advanced.section_title": "Paramètres avancés",
};

const DE_TRANSLATIONS: TranslationDictionary = {
  "nav.home": "Start",
  "nav.discover": "Inhalte entdecken",
  "nav.creator_studio": "Creator Studio",
  "nav.library": "Bibliothek",
  "nav.updates": "Verfügbare Updates",
  "nav.skins": "Skins",
  "nav.create_instance": "Neue Instanz erstellen",
  "nav.dev": "Dev",
  "nav.account": "Konto",
  "nav.settings": "Einstellungen",
  "settings.title": "Einstellungen",
  "settings.intro": "Aussehen, Konto und Launcher-Verhalten.",
  "settings.mode.basic": "Einfacher Modus",
  "settings.mode.advanced": "Erweiterter Modus",
  "settings.appearance.section_title": "Aussehen",
  "settings.appearance.section_sub": "Passe das Aussehen der App an, ohne das Layout zu verändern.",
  "settings.appearance.theme.title": "Thema",
  "settings.appearance.theme.sub": "Zwischen dunkel und hell wechseln.",
  "settings.appearance.theme.dark": "Dunkel",
  "settings.appearance.theme.light": "Hell",
  "settings.appearance.accent.title": "Akzent",
  "settings.appearance.accent.sub": "Wähle einen Akzent. Neutral bleibt dezent, Farben sind kräftiger.",
  "settings.appearance.accent_strength.title": "Akzentstärke",
  "settings.appearance.accent_strength.sub": "Passe Deckkraft und Intensität des Akzents von dezent bis maximal an.",
  "settings.appearance.motion.title": "Bewegungsprofil",
  "settings.appearance.motion.sub": "Wähle, wie animiert sich die Oberfläche anfühlen soll.",
  "settings.appearance.density.title": "UI-Dichte",
  "settings.appearance.density.sub": "Komfortabel lässt mehr Platz, kompakt zeigt mehr auf einmal.",
  "settings.appearance.reset.title": "UI-Einstellungen zurücksetzen",
  "settings.appearance.reset.sub":
    "Setzt Thema, Akzent, Akzentstärke, Bewegungsprofil und Dichte auf die Standardwerte zurück.",
  "settings.appearance.reset.button": "Aussehen zurücksetzen",
  "settings.language.section_title": "Sprache",
  "settings.language.section_sub": "Wähle die Sprache für OpenJar Launcher.",
  "settings.language.preference.title": "App-Sprache",
  "settings.language.preference.sub":
    "Änderungen gelten sofort und fallen bei nicht übersetzten Texten auf Englisch zurück.",
  "settings.language.preference.menu_prefix": "Sprache",
  "settings.language.warning":
    "Diese erste Lokalisierungsrunde deckt Navigation und das wichtigste Einstellungen-Erlebnis ab. Nicht übersetzte Bereiche bleiben vorerst Englisch.",
  "settings.language.saving": "Sprache wird gespeichert…",
  "settings.language.saved_notice": "App-Sprache wurde auf {language} umgestellt.",
  "settings.launch.section_title": "Startkonfiguration",
  "settings.launch.section_sub": "Lege Standardverhalten des Launchers und die Java-Laufzeit fest.",
  "settings.launch.method.title": "Standard-Startmethode",
  "settings.launch.method.sub": "Verwende standardmäßig den nativen Launcher oder Prism.",
  "settings.launch.method.native": "Nativ",
  "settings.launch.method.prism": "Prism",
  "settings.launch.java.title": "Java-Executable",
  "settings.launch.java.sub":
    "Absoluter Pfad zu Java, oder leer lassen, um `java` aus dem PATH zu verwenden. Minecraft 1.20.5+ benötigt Java 21+.",
  "settings.launch.java.browse": "Durchsuchen…",
  "settings.launch.java.detect": "Installiertes Java erkennen",
  "settings.launch.java.detecting": "Erkenne…",
  "settings.launch.java.get_java_21": "Java 21 holen",
  "settings.launch.java.selected": "Ausgewählt",
  "settings.launch.java.use": "Verwenden",
  "settings.launch.oauth.title": "OAuth-Client-ID überschreiben",
  "settings.launch.oauth.sub":
    "Die Client-ID ist ein öffentlicher Bezeichner, kein geheimer API-Schlüssel. Leer lassen, um den integrierten Standard zu verwenden.",
  "settings.launch.oauth.placeholder": "Optionale Client-ID",
  "settings.launch.basic_hidden": "Erweiterte Java- und OAuth-Optionen sind im einfachen Modus ausgeblendet.",
  "settings.launch.switch_to_advanced": "Zu erweitert wechseln",
  "settings.launch.save": "Launcher-Einstellungen speichern",
  "settings.launch.saving": "Speichern…",
  "settings.launch.saved_notice": "Launcher-Einstellungen gespeichert.",
  "settings.account.section_title": "Microsoft-Konto",
  "settings.account.section_sub":
    "Verbinde das Microsoft-Konto, dem Minecraft gehört. Normalerweise musst du keine Client-ID konfigurieren.",
  "settings.updates.section_title": "App-Updates",
  "settings.content.section_title": "Inhalte und Visuals",
  "settings.advanced.section_title": "Erweiterte Einstellungen",
};

const PT_TRANSLATIONS: TranslationDictionary = {
  "nav.home": "Início",
  "nav.discover": "Descobrir conteúdo",
  "nav.creator_studio": "Creator Studio",
  "nav.library": "Biblioteca",
  "nav.updates": "Atualizações disponíveis",
  "nav.skins": "Skins",
  "nav.create_instance": "Criar nova instância",
  "nav.dev": "Dev",
  "nav.account": "Conta",
  "nav.settings": "Configurações",
  "settings.title": "Configurações",
  "settings.intro": "Aparência, conta e comportamento do launcher.",
  "settings.mode.basic": "Modo básico",
  "settings.mode.advanced": "Modo avançado",
  "settings.appearance.section_title": "Aparência",
  "settings.appearance.section_sub": "Ajuste o visual do app sem mudar o comportamento do layout.",
  "settings.appearance.theme.title": "Tema",
  "settings.appearance.theme.sub": "Alterne entre escuro e claro.",
  "settings.appearance.theme.dark": "Escuro",
  "settings.appearance.theme.light": "Claro",
  "settings.appearance.accent.title": "Acento",
  "settings.appearance.accent.sub": "Escolha um acento. Neutral continua sutil, enquanto as cores ficam mais fortes.",
  "settings.appearance.accent_strength.title": "Intensidade do acento",
  "settings.appearance.accent_strength.sub": "Ajuste a opacidade e a intensidade do acento de sutil ao máximo.",
  "settings.appearance.motion.title": "Perfil de movimento",
  "settings.appearance.motion.sub": "Escolha o quanto a interface deve ser animada.",
  "settings.appearance.density.title": "Densidade da interface",
  "settings.appearance.density.sub": "Confortável deixa mais espaço; compacto mostra mais na tela.",
  "settings.appearance.reset.title": "Redefinir ajustes visuais",
  "settings.appearance.reset.sub":
    "Restaura tema, acento, intensidade do acento, perfil de movimento e densidade para os padrões.",
  "settings.appearance.reset.button": "Redefinir aparência",
  "settings.language.section_title": "Idioma",
  "settings.language.section_sub": "Escolha o idioma usado pelo OpenJar Launcher.",
  "settings.language.preference.title": "Idioma do app",
  "settings.language.preference.sub":
    "As mudanças são aplicadas imediatamente e usam inglês como fallback para qualquer texto ainda não traduzido.",
  "settings.language.preference.menu_prefix": "Idioma",
  "settings.language.warning":
    "Esta primeira etapa de localização cobre a navegação e a experiência principal de Configurações. As áreas ainda não traduzidas continuam em inglês por enquanto.",
  "settings.language.saving": "Salvando idioma…",
  "settings.language.saved_notice": "Idioma do app atualizado para {language}.",
  "settings.launch.section_title": "Configuração de inicialização",
  "settings.launch.section_sub": "Defina o comportamento padrão do launcher e o runtime de Java.",
  "settings.launch.method.title": "Método de inicialização padrão",
  "settings.launch.method.sub": "Use o launcher nativo ou o Prism por padrão.",
  "settings.launch.method.native": "Nativo",
  "settings.launch.method.prism": "Prism",
  "settings.launch.java.title": "Executável do Java",
  "settings.launch.java.sub":
    "Caminho absoluto para o Java, ou deixe em branco para usar `java` do PATH. Minecraft 1.20.5+ precisa de Java 21+.",
  "settings.launch.java.browse": "Procurar…",
  "settings.launch.java.detect": "Detectar Java instalado",
  "settings.launch.java.detecting": "Detectando…",
  "settings.launch.java.get_java_21": "Obter Java 21",
  "settings.launch.java.selected": "Selecionado",
  "settings.launch.java.use": "Usar",
  "settings.launch.oauth.title": "Sobrescrever ID do cliente OAuth",
  "settings.launch.oauth.sub":
    "O ID do cliente é um identificador público, não uma chave secreta. Deixe em branco para usar o padrão embutido.",
  "settings.launch.oauth.placeholder": "ID do cliente opcional",
  "settings.launch.basic_hidden": "As substituições avançadas de Java e OAuth ficam ocultas no modo básico.",
  "settings.launch.switch_to_advanced": "Mudar para avançado",
  "settings.launch.save": "Salvar configurações do launcher",
  "settings.launch.saving": "Salvando…",
  "settings.launch.saved_notice": "Configurações do launcher salvas.",
  "settings.account.section_title": "Conta Microsoft",
  "settings.account.section_sub":
    "Conecte a conta Microsoft que possui o Minecraft. Normalmente você não precisa configurar nenhum ID de cliente.",
  "settings.updates.section_title": "Atualizações do app",
  "settings.content.section_title": "Conteúdo e visual",
  "settings.advanced.section_title": "Configurações avançadas",
};

const APP_TRANSLATIONS: Record<AppLanguage, TranslationDictionary> = {
  "en-US": EN_TRANSLATIONS,
  "es-ES": ES_TRANSLATIONS,
  "fr-FR": FR_TRANSLATIONS,
  "de-DE": DE_TRANSLATIONS,
  "pt-BR": PT_TRANSLATIONS,
};

export function normalizeAppLanguage(input?: string | null): AppLanguage {
  const normalized = String(input ?? "").trim().toLowerCase();
  switch (normalized) {
    case "es":
    case "es-es":
    case "es-419":
    case "spanish":
      return "es-ES";
    case "fr":
    case "fr-fr":
    case "french":
      return "fr-FR";
    case "de":
    case "de-de":
    case "german":
      return "de-DE";
    case "pt":
    case "pt-br":
    case "portuguese":
      return "pt-BR";
    case "en":
    case "en-us":
    case "english":
    default:
      return "en-US";
  }
}

export function getAppLanguageOption(language: AppLanguage): AppLanguageOption {
  return APP_LANGUAGE_OPTIONS.find((option) => option.value === language) ?? APP_LANGUAGE_OPTIONS[0];
}

export function translateAppText(
  language: AppLanguage,
  key: AppTranslationKey,
  vars?: Record<string, string | number>
): string {
  const source = APP_TRANSLATIONS[language]?.[key] ?? EN_TRANSLATIONS[key];
  if (!vars) return source;
  return source.replace(/\{(\w+)\}/g, (_, token: string) => {
    const value = vars[token];
    return value === undefined ? `{${token}}` : String(value);
  });
}
